//! Provides the dispatch functionality.
use crate::config::Settings;
use crate::model::{Event, EventKind, Session, ToolCall};
use serde_json::Value;
use std::io;

/// Executes requested tool calls. Each call is stored, then on-event and the
/// on-tool-call gate run; a block (hook exit 2) replaces the tool's result with
/// an error the model can react to. Tool failures are likewise fed back to the
/// model rather than failing the turn.
pub fn execute<F: FnMut(&Session) -> Option<String>>(
    settings: &Settings,
    session: &mut Session,
    calls: Vec<(String, Value)>,
    progress: &mut F,
) -> io::Result<()> {
    for (name, input) in calls {
        let call_id = uuid::Uuid::new_v4().to_string();
        append_call(session, &call_id, &name, &input);
        let blocked = progress(session).or_else(|| {
            let call = session.events.last()?;
            crate::hooks::fire(settings, "on-tool-call", call, session)
        });
        let result = match blocked {
            Some(reason) => serde_json::json!({"error": format!("blocked by hook: {reason}")}),
            None => invoke(settings, session, &name, &input)
                .unwrap_or_else(|error| serde_json::json!({"error": error.to_string()})),
        };
        append_result(session, call_id, result);
        super::lifecycle::check(progress(session))?;
    }
    Ok(())
}

/// Performs the append call operation.
fn append_call(session: &mut Session, call_id: &str, name: &str, input: &Value) {
    let call = ToolCall {
        call_id: call_id.into(),
        name: name.into(),
        input: input.clone(),
    };
    session.append(Event::new(EventKind::ToolCall { calls: vec![call] }));
}

/// Performs the append result operation.
fn append_result(session: &mut Session, call_id: String, content: Value) {
    let content = crate::tools::clip(content);
    session.append(Event::new(EventKind::ToolResult { call_id, content }));
}

/// Invokes a named tool.
fn invoke(settings: &Settings, session: &Session, name: &str, input: &Value) -> io::Result<Value> {
    if !input.is_object() {
        let message = format!("tool arguments must be a JSON object, got: {input}");
        return Err(io::Error::new(io::ErrorKind::InvalidInput, message));
    }
    match name {
        "bash" => crate::tools::bash_in(
            settings,
            input.get("cmd").and_then(Value::as_str).unwrap_or_default(),
            working_dir(settings, session),
        ),
        "fs_read" => {
            crate::tools::read_from(settings, &paths(input), working_dir(settings, session))
        }
        "fs_edit" => crate::tools::edit_from(
            settings,
            input
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            input.get("edits").unwrap_or(&Value::Null),
            working_dir(settings, session),
        ),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unknown provider tool",
        )),
    }
}

/// Resolves the working directory.
fn working_dir<'a>(settings: &'a Settings, session: &'a Session) -> &'a std::path::Path {
    session
        .metadata
        .working_dir
        .as_deref()
        .map(std::path::Path::new)
        .unwrap_or(&settings.root)
}

/// Extracts paths from input.
fn paths(input: &Value) -> Vec<String> {
    input
        .get("paths")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}
