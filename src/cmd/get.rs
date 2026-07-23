use crate::session::{is_id_format, SessionStore};
use serde_json::{Map, Value};
use std::fs;
use std::path::Path;

pub fn get(
    id: &str,
    conversation: bool,
    last_message: bool,
    metadata: bool,
    state: bool,
    config_dir: &Path,
) -> Result<Value, String> {
    if !is_id_format(id) {
        return Err(format!("invalid session ID: '{}'", id));
    }
    let store = SessionStore::with_base(config_dir.join("sessions"));
    let mut result = Map::new();
    if conversation || last_message {
        let events = read_jsonl(&store.transcript_path(id), "conversation")?;
        if conversation {
            result.insert("conversation".to_string(), events.clone());
        }
        if last_message {
            result.insert("last_message".to_string(), latest_assistant(&events));
        }
    }
    if metadata {
        result.insert("metadata".to_string(), read_json(&store.metadata_path(id), "metadata")?);
    }
    if state {
        result.insert("state".to_string(), read_json(&store.state_path(id), "state")?);
    }
    Ok(Value::Object(result))
}

fn latest_assistant(events: &Value) -> Value {
    events
        .as_array()
        .and_then(|events| events.iter().rev().find(|event| {
            event["type"] == "message" && event["message"]["role"] == "assistant"
        }))
        .cloned()
        .unwrap_or(Value::Null)
}

fn read_json(path: &Path, resource: &str) -> Result<Value, String> {
    let contents = fs::read_to_string(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound { format!("session {} is missing", resource) }
        else { format!("failed to read {}: {}", resource, e) }
    })?;
    serde_json::from_str(&contents).map_err(|e| format!("invalid {} JSON: {}", resource, e))
}

fn read_jsonl(path: &Path, resource: &str) -> Result<Value, String> {
    let contents = fs::read_to_string(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound { format!("session {} is missing", resource) }
        else { format!("failed to read {}: {}", resource, e) }
    })?;
    let mut events = Vec::new();
    for (line, text) in contents.lines().enumerate() {
        if text.trim().is_empty() { continue; }
        events.push(serde_json::from_str(text).map_err(|e| format!("invalid {} JSON on line {}: {}", resource, line + 1, e))?);
    }
    Ok(Value::Array(events))
}
