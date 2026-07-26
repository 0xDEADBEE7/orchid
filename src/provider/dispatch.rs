use super::Settings;
use crate::model::{Event, Session};
use serde_json::Value;
use std::io;

pub fn execute<F: FnMut(&Session)>(
    settings: &Settings,
    session: &mut Session,
    calls: Vec<(String, Value)>,
    mut progress: F,
) -> io::Result<String> {
    let mut output = Vec::new();
    for (name, input) in calls {
        let call_id = uuid::Uuid::new_v4().to_string();
        session.append(Event::ToolCall {
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            calls: vec![crate::model::ToolCall {
                call_id: call_id.clone(),
                name: name.clone(),
                input: input.clone(),
            }],
        });
        progress(session);
        match invoke(settings, session, &name, &input) {
            Ok(value) => {
                output.push(value.to_string());
                session.append(Event::ToolResult {
                    event_id: uuid::Uuid::new_v4().to_string(),
                    timestamp: chrono::Utc::now(),
                    call_id,
                    content: value,
                });
                progress(session);
            }
            Err(error) => {
                session.append(Event::ToolResult {
                    event_id: uuid::Uuid::new_v4().to_string(),
                    timestamp: chrono::Utc::now(),
                    call_id,
                    content: serde_json::json!({"error":error.to_string()}),
                });
                progress(session);
                return Err(error);
            }
        }
    }
    Ok(output.join("\n"))
}

fn invoke(settings: &Settings, session: &Session, name: &str, input: &Value) -> io::Result<Value> {
    match name {
        "bash" => crate::tools::bash_in(
            settings,
            input.get("cmd").and_then(Value::as_str).unwrap_or_default(),
            session
                .metadata
                .working_dir
                .as_deref()
                .map(std::path::Path::new)
                .unwrap_or(&settings.root),
        ),
        "fs_read" => crate::tools::read_from(
            settings,
            &input
                .get("paths")
                .and_then(Value::as_array)
                .map(|paths| {
                    paths
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default(),
            session
                .metadata
                .working_dir
                .as_deref()
                .map(std::path::Path::new)
                .unwrap_or(&settings.root),
        ),
        "fs_edit" => crate::tools::edit_from(
            settings,
            input
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            input.get("edits").unwrap_or(&Value::Null),
            session
                .metadata
                .working_dir
                .as_deref()
                .map(std::path::Path::new)
                .unwrap_or(&settings.root),
        ),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unknown provider tool",
        )),
    }
}
