use super::Settings;
use crate::model::{Event, Session};
use serde_json::Value;
use std::io;

pub fn execute<F: FnMut(&Session)>(
    settings: &Settings,
    session: &mut Session,
    calls: Vec<(String, Value)>,
    progress: F,
) -> io::Result<String> {
    execute_with_ids(
        settings,
        session,
        calls.into_iter().map(|(name, input)| (name, input, None)),
        progress,
    )
}

pub fn execute_with_ids<
    F: FnMut(&Session),
    I: IntoIterator<Item = (String, Value, Option<String>)>,
>(
    settings: &Settings,
    session: &mut Session,
    calls: I,
    mut progress: F,
) -> io::Result<String> {
    let mut output = Vec::new();
    for (name, input, requested_id) in calls {
        let call_id = requested_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        append_call(session, &call_id, &name, &input);
        progress(session);
        match invoke(settings, session, &name, &input) {
            Ok(value) => {
                output.push(value.to_string());
                append_result(session, call_id, value);
                progress(session);
            }
            Err(error) => {
                append_result(
                    session,
                    call_id,
                    serde_json::json!({"error":error.to_string()}),
                );
                progress(session);
                // Tool failures are part of the model interaction, rather than
                // failures of the worker itself. Feed the structured error back
                // to the provider so it can repair malformed arguments, choose
                // another tool, or explain the problem to the user.
            }
        }
    }
    Ok(output.join("\n"))
}

fn append_call(session: &mut Session, call_id: &str, name: &str, input: &Value) {
    session.append(Event::ToolCall {
        event_id: uuid::Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now(),
        calls: vec![crate::model::ToolCall {
            call_id: call_id.into(),
            name: name.into(),
            input: input.clone(),
        }],
        token_usage: crate::model::TokenUsage::default(),
    });
}

fn append_result(session: &mut Session, call_id: String, content: Value) {
    session.append(Event::ToolResult {
        event_id: uuid::Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now(),
        call_id,
        content,
        token_usage: crate::model::TokenUsage::default(),
    });
}

fn invoke(settings: &Settings, session: &Session, name: &str, input: &Value) -> io::Result<Value> {
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

fn working_dir<'a>(settings: &'a Settings, session: &'a Session) -> &'a std::path::Path {
    session
        .metadata
        .working_dir
        .as_deref()
        .map(std::path::Path::new)
        .unwrap_or(&settings.root)
}

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
