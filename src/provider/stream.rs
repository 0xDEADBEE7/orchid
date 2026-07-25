use super::{parse_sse, HttpProvider, Session, StreamEvent};
use crate::provider::{defs::codex_tools, tool_definitions};
use serde_json::{json, Value};
use std::io::{self, Read};

pub fn run(
    provider: &HttpProvider,
    prompt: &str,
    session: &Session,
) -> io::Result<Vec<StreamEvent>> {
    let is_codex = provider.connection.interface == "codex"
        || matches!(
            provider.credential,
            Some(crate::config::Credential::Codex { .. })
        );
    let body = if is_codex {
        let mut body = json!({"model":provider.connection.model,"instructions":provider.system_prompt,"input":codex_input(session, prompt),"tools":codex_tools(&provider.tools),"store":false,"stream":true});
        merge_params(&mut body, &provider.params);
        body
    } else if provider.connection.interface == "anthropic" {
        json!({"model":provider.connection.model,"max_tokens":1024,"stream":true,"messages":[{"role":"user","content":prompt}]})
    } else {
        json!({"model":provider.connection.model,"stream":true,"messages":[{"role":"system","content":provider.system_prompt},{"role":"user","content":prompt}],"tools":tool_definitions(&provider.tools)})
    };
    let mut response = provider.transport()?.request(provider, &body, true)?;
    let mut raw = String::new();
    response
        .read_to_string(&mut raw)
        .map_err(io::Error::other)?;
    Ok(parse_sse(&raw))
}

fn merge_params(body: &mut Value, params: &std::collections::HashMap<String, Value>) {
    if let Some(object) = body.as_object_mut() {
        for (name, value) in params {
            object.insert(name.clone(), value.clone());
        }
    }
}

fn codex_input(session: &Session, prompt: &str) -> Vec<Value> {
    let mut input = Vec::new();
    for event in &session.events {
        match event {
            crate::model::Event::Message { role, content, .. } => {
                let content_type = if role == "assistant" {
                    "output_text"
                } else {
                    "input_text"
                };
                input.push(json!({
                    "role": role,
                    "content": [{"type": content_type, "text": content}]
                }));
            }
            crate::model::Event::ToolCall { calls, .. } => input.extend(calls.iter().map(|call| {
                json!({
                    "type": "function_call", "call_id": call.call_id,
                    "name": call.name, "arguments": call.input.to_string()
                })
            })),
            crate::model::Event::ToolResult {
                call_id, content, ..
            } => input.push(json!({
                "type": "function_call_output", "call_id": call_id,
                "output": content.to_string()
            })),
            _ => {}
        }
    }
    if input.is_empty() {
        input.push(json!({"role":"user","content":[{"type":"input_text","text":prompt}]}));
    }
    input
}
