use crate::log::{DiagLogger, LogReader};
use crate::tools::fs_read::extract_paths;
use crate::types::{ConvoEvent, Message, ToolResult};
use serde_json::Value;
use std::collections::HashMap;

pub fn build_message_history(
    convo_id: &str,
    config_dir: &std::path::Path,
    log: &DiagLogger,
) -> Result<Vec<Message>, String> {
    let path = config_dir
        .join("sessions")
        .join(convo_id)
        .join("conversation.jsonl");

    if !std::path::Path::new(&path).exists() {
        return Ok(Vec::new());
    }

    let all_events = LogReader::read_lines(&path)?;

    // Pass 1: find the last index at which each file path was read.
    let mut last_read: HashMap<String, usize> = HashMap::new();

    for (idx, event) in all_events.iter().enumerate() {
        if let ConvoEvent::ToolCall(e) = event {
            for tc in &e.tool_call.calls {
                if tc.name == "fs_read" {
                    for p in extract_paths(&tc.input) {
                        last_read.insert(p, idx);
                    }
                }
            }
        }
    }

    // Pass 2: build the message slice, substituting stale content markers in-memory.
    let mut call_map: HashMap<String, (String, serde_json::Value)> = HashMap::new();
    let mut messages = Vec::new();
    let mut raw_messages = Vec::new();
    let mut tombstone_count: u32 = 0;

    for (idx, event) in all_events.iter().enumerate() {
        match event {
            ConvoEvent::Message(e) => {
                if e.message.role != "system" {
                    let msg = Message {
                        role: e.message.role.clone(),
                        content: e.message.content.clone(),
                        tool_calls: None,
                        tool_result: None,
                    };
                    raw_messages.push(msg.clone());
                    messages.push(msg);
                }
            }
            ConvoEvent::ToolCall(e) => {
                for tc in &e.tool_call.calls {
                    call_map.insert(tc.id.clone(), (tc.name.clone(), tc.input.clone()));
                }
                let msg = Message {
                    role: "assistant".to_string(),
                    content: String::new(),
                    tool_calls: Some(e.tool_call.calls.clone()),
                    tool_result: None,
                };
                raw_messages.push(msg.clone());
                messages.push(msg);
            }
            ConvoEvent::ToolResult(e) => {
                let tr = &e.tool_result;
                let raw_msg = Message {
                    role: "user".to_string(),
                    content: String::new(),
                    tool_calls: None,
                    tool_result: Some(tr.clone()),
                };

                if let Some((name, input)) = call_map.get(&tr.call_id) {
                    if name == "fs_read" {
                        let paths = extract_paths(input);
                        let stale_paths: Vec<&String> = paths
                            .iter()
                            .filter(|p| last_read.get(*p).is_some_and(|&last| last > idx))
                            .collect();

                        if !stale_paths.is_empty() {
                            tombstone_count += 1;
                            raw_messages.push(raw_msg);
                            messages.push(Message {
                                role: "user".to_string(),
                                content: String::new(),
                                tool_calls: None,
                                tool_result: Some(ToolResult {
                                    call_id: tr.call_id.clone(),
                                    content: replace_stale_in_value(&tr.content, &stale_paths),
                                }),
                            });
                            continue;
                        }
                    }
                }
                raw_messages.push(raw_msg.clone());
                messages.push(raw_msg);
            }
            ConvoEvent::Reasoning(_) => {
                // Reasoning content is persisted in the file for observability,
                // but omitted from history since it's internal to the model.
            }
        }
    }

    if tombstone_count > 0 {
        let raw_tokens = estimate_tokens_from_messages(&raw_messages);
        let effective_tokens = estimate_tokens_from_messages(&messages);
        log.debug(
            "tombstone_savings",
            &format!(
                "tombstones={} raw_tokens={} effective_tokens={} saved={}",
                tombstone_count,
                raw_tokens,
                effective_tokens,
                raw_tokens.saturating_sub(effective_tokens),
            ),
        );
    }

    Ok(messages)
}

pub fn estimate_tokens_from_messages(messages: &[Message]) -> u32 {
    let bytes = serde_json::to_string(messages)
        .map(|s| s.len())
        .unwrap_or(0);
    (bytes / 3) as u32
}

/// Replace stale path entries in a tool result `Value` with `{"stale": true}`.
/// Content is expected to be a JSON object `{"<path>": <value>, ...}`.
/// Paths not present in the object are silently ignored.
/// Falls back to the original value unchanged if it is not an object.
pub fn replace_stale_in_value(content: &Value, stale_paths: &[&String]) -> Value {
    let Value::Object(map) = content else {
        return content.clone();
    };

    let mut map = map.clone();
    for path in stale_paths {
        if map.contains_key(path.as_str()) {
            map.insert(path.to_string(), serde_json::json!({"stale": true}));
        }
    }
    Value::Object(map)
}
