use super::{Client, ClientError, ClientErrorKind, ClientEvent, ClientRequest};
use crate::{
    config::{Credential, ResolvedConnection},
    model::{ToolCall, Usage},
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub struct OpenAiChatClient {
    connection: ResolvedConnection,
}

impl OpenAiChatClient {
    pub fn new(connection: ResolvedConnection) -> Self {
        Self { connection }
    }
}

impl Client for OpenAiChatClient {
    fn stream(&self, request: ClientRequest) -> Result<Vec<ClientEvent>, ClientError> {
        let mut call = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(15))
            .timeout(None)
            .build()
            .map_err(|error| ClientError::new(ClientErrorKind::Transport, error.to_string()))?
            .post(endpoint(&self.connection.connection.base_url))
            .header("Accept", "text/event-stream")
            .json(&request_body(request));
        if let Some(Credential::ApiKey(key)) = &self.connection.credential {
            call = call.bearer_auth(key);
        }
        for (name, value) in &self.connection.headers {
            call = call.header(name, value);
        }
        let response = call.send().map_err(|error| {
            ClientError::new(
                ClientErrorKind::Transport,
                format!("OpenAI request failed: {error}"),
            )
        })?;
        let status = response.status();
        if !status.is_success() {
            let kind = if status.as_u16() == 401 || status.as_u16() == 403 {
                ClientErrorKind::Authentication
            } else {
                ClientErrorKind::Http
            };
            let detail = response.text().unwrap_or_default();
            return Err(ClientError::new(
                kind,
                format!("OpenAI HTTP status {status}: {detail}"),
            ));
        }
        let text = response
            .text()
            .map_err(|error| ClientError::new(ClientErrorKind::Transport, error.to_string()))?;
        parse_sse(&text)
    }
}

fn endpoint(base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    if base.ends_with("/chat/completions") {
        base.into()
    } else {
        format!("{base}/v1/chat/completions")
    }
}

fn request_body(request: ClientRequest) -> Value {
    let mut messages = Vec::new();
    if !request.system_prompt.is_empty() {
        messages.push(json!({"role":"system", "content":request.system_prompt}));
    }
    for message in request.messages {
        if let Some((call_id, content)) = message.tool_result {
            messages.push(
                json!({"role":"tool", "tool_call_id":call_id, "content":content.to_string()}),
            );
        } else if !message.tool_calls.is_empty() {
            let calls = message
                .tool_calls
                .into_iter()
                .map(|call| {
                    json!({
                        "id":call.call_id, "type":"function",
                        "function":{"name":call.name, "arguments":call.input.to_string()}
                    })
                })
                .collect::<Vec<_>>();
            messages
                .push(json!({"role":message.role, "content":message.content, "tool_calls":calls}));
        } else {
            messages.push(json!({"role":message.role, "content":message.content}));
        }
    }
    let mut body = json!({"model":request.model, "messages":messages, "stream":true});
    if !request.tools.is_empty() {
        body["tools"] = Value::Array(request.tools.into_iter().map(|tool| json!({
            "type":"function",
            "function":{"name":tool.name,"description":tool.description,"parameters":tool.parameters}
        })).collect());
    }
    if let Some(object) = body.as_object_mut() {
        object.extend(request.params);
    }
    body
}

#[derive(Default)]
struct PendingCall {
    id: String,
    name: String,
    arguments: String,
}

pub fn parse_sse(input: &str) -> Result<Vec<ClientEvent>, ClientError> {
    let mut events = Vec::new();
    let mut calls: BTreeMap<u64, PendingCall> = BTreeMap::new();
    for line in input.lines() {
        let Some(data) = line.strip_prefix("data:").map(str::trim) else {
            continue;
        };
        if data == "[DONE]" {
            continue;
        }
        let value: Value = serde_json::from_str(data).map_err(|error| {
            ClientError::new(
                ClientErrorKind::Protocol,
                format!("invalid OpenAI stream event: {error}"),
            )
        })?;
        if let Some(usage) = value.get("usage").filter(|usage| !usage.is_null()) {
            events.push(ClientEvent::Usage(Usage {
                input: u32_value(usage.get("prompt_tokens")),
                output: u32_value(usage.get("completion_tokens")),
                cached_input: u32_value(usage.pointer("/prompt_tokens_details/cached_tokens")),
            }));
        }
        let Some(delta) = value.pointer("/choices/0/delta") else {
            continue;
        };
        if let Some(content) = delta.get("content").and_then(Value::as_str) {
            events.push(ClientEvent::TextDelta(content.into()));
        }
        if let Some(reasoning) = delta.get("reasoning_content").and_then(Value::as_str) {
            events.push(ClientEvent::ReasoningDelta(reasoning.into()));
        }
        if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
            for chunk in tool_calls {
                let index = chunk.get("index").and_then(Value::as_u64).unwrap_or(0);
                let call = calls.entry(index).or_default();
                if let Some(id) = chunk.get("id").and_then(Value::as_str) {
                    call.id.push_str(id);
                }
                if let Some(name) = chunk.pointer("/function/name").and_then(Value::as_str) {
                    call.name.push_str(name);
                }
                if let Some(args) = chunk.pointer("/function/arguments").and_then(Value::as_str) {
                    call.arguments.push_str(args);
                }
            }
        }
    }
    for (_, call) in calls {
        let input = serde_json::from_str(&call.arguments).map_err(|error| {
            ClientError::new(
                ClientErrorKind::Protocol,
                format!("invalid tool arguments: {error}"),
            )
        })?;
        events.push(ClientEvent::ToolCall(ToolCall {
            call_id: call.id,
            name: call.name,
            input,
        }));
    }
    events.push(ClientEvent::Done);
    Ok(events)
}

fn u32_value(value: Option<&Value>) -> u32 {
    value
        .and_then(Value::as_u64)
        .and_then(|number| number.try_into().ok())
        .unwrap_or(0)
}
