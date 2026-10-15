//! Provides the mod functionality.
pub mod auth;
pub mod compatibility;

use super::{Client, ClientError, ClientErrorKind, ClientEvent, ClientRequest};
use crate::config::ResolvedConnection;
use crate::model::{ToolCall, Usage};
use serde_json::json;
use std::io::{BufRead, BufReader};
use uuid::Uuid;

/// Performs the CodexClient operation.
pub struct CodexClient {
    connection: ResolvedConnection,
}

/// Reads SSE lines until the server closes the stream. Blank lines and `:`
/// keepalives are skipped; an in-stream failure ends the read with its error.
fn read_stream(reader: impl BufRead) -> Result<Vec<ClientEvent>, ClientError> {
    let mut events = Vec::new();
    for line in reader.lines() {
        let line = line.map_err(|error| {
            ClientError::new(
                ClientErrorKind::Transport,
                format!("Codex stream read failed: {error}"),
            )
        })?;
        let Some(data) = line.strip_prefix("data:").map(str::trim) else {
            continue;
        };
        let value = serde_json::from_str(data).unwrap_or_default();
        if let Some(error) = stream_failure(&value) {
            return Err(error);
        }
        events.extend(parse_payload(data));
    }
    if events.contains(&ClientEvent::Done) {
        return Ok(events);
    }
    // The connection closed without a completion or failure event.
    let message = "Codex stream ended before completion";
    Err(ClientError::new(ClientErrorKind::Transport, message))
}

/// Classifies an in-stream failure (`error` or `response.failed`) by its code, so
/// overload and server errors are retried but e.g. context-length errors are not.
fn stream_failure(value: &serde_json::Value) -> Option<ClientError> {
    let error = match value["type"].as_str()? {
        "error" => value
            .get("error")
            .filter(|e| e.is_object())
            .unwrap_or(value),
        "response.failed" => value.pointer("/response/error")?,
        _ => return None,
    };
    let code = error["code"].as_str().unwrap_or("unknown");
    let message = error["message"].as_str().unwrap_or("no message");
    Some(ClientError::new(
        super::base::error_kind(error),
        format!("Codex response failed: {message} (code: {code})"),
    ))
}

impl CodexClient {
    /// Creates a new value.
    pub fn new(connection: ResolvedConnection) -> Self {
        Self { connection }
    }
}
impl Client for CodexClient {
    /// Streams events for a request.
    fn stream(&self, request: ClientRequest) -> Result<Vec<ClientEvent>, ClientError> {
        let credential = self.connection.credential.as_ref().ok_or_else(|| {
            ClientError::new(
                ClientErrorKind::Authentication,
                "Codex credential unavailable",
            )
        })?;
        let (token, account) = auth::CodexAuth::present(credential)?;
        let mut body = request_body(&request);
        merge_params(&mut body, request.params);
        let response = send_request(&self.connection, token, account, &body)?;
        if !response.status().is_success() {
            return Err(super::base::http_error("Codex", response));
        }
        read_stream(BufReader::new(response))
    }
}

/// Performs the request body operation.
fn request_body(request: &ClientRequest) -> serde_json::Value {
    json!({
        "model": request.model,
        "instructions": request.system_prompt,
        "input": request.messages.iter().flat_map(|m| {
            if !m.tool_calls.is_empty() {
                return m.tool_calls.iter().map(|call| json!({"type":"function_call","call_id":call.call_id,"name":call.name,"arguments":call.input.to_string()})).collect::<Vec<_>>();
            }
            if let Some(call_id) = &m.tool_call_id {
                return vec![json!({"type":"function_call_output","call_id":call_id,"output":m.content})];
            }
            vec![json!({"role":m.role,"content":[{"type":if m.role == "assistant" {"output_text"} else {"input_text"},"text":m.content}]})]
        }).collect::<Vec<_>>(),
        "tools": request.tools.iter().map(|tool| json!({"type":"function","name":tool.name,"description":tool.description,"parameters":tool.parameters})).collect::<Vec<_>>(),
        "store": false,
        "stream": true
    })
}

/// Performs the send request operation.
fn send_request(
    connection: &ResolvedConnection,
    token: &str,
    account: &str,
    body: &serde_json::Value,
) -> Result<reqwest::blocking::Response, ClientError> {
    let url = format!(
        "{}/responses",
        connection.connection.base_url.trim_end_matches('/')
    );
    let http = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(None)
        .build()
        .map_err(|e| ClientError::new(ClientErrorKind::Transport, e.to_string()))?;
    let mut call = http
        .post(url)
        .bearer_auth(token)
        .header("ChatGPT-Account-ID", account)
        .header("originator", "codex_cli_rs")
        .header("openai-beta", "responses=experimental")
        .header("Version", compatibility::VERSION)
        .header("Session_Id", Uuid::new_v4().to_string())
        .header("User-Agent", compatibility::USER_AGENT)
        .header("Accept", "text/event-stream")
        .header("Content-Type", "application/json")
        .json(body);
    for (name, value) in &connection.headers {
        call = call.header(name, value);
    }
    call.send().map_err(|e| {
        ClientError::new(
            ClientErrorKind::Transport,
            format!("Codex request dispatch failed: {e}"),
        )
    })
}

/// Performs the merge params operation.
fn merge_params(body: &mut serde_json::Value, params: serde_json::Map<String, serde_json::Value>) {
    if let Some(object) = body.as_object_mut() {
        object.extend(params);
    }
}

/// Parses server-sent events into client events.
pub fn parse_sse(input: &str) -> Vec<ClientEvent> {
    payloads(input)
        .into_iter()
        .flat_map(parse_payload)
        .collect()
}

/// Performs the payloads operation.
fn payloads(input: &str) -> Vec<&str> {
    if input.trim_start().starts_with('{') {
        vec![input.trim()]
    } else {
        input
            .lines()
            .filter_map(|line| line.strip_prefix("data:").map(str::trim))
            .collect()
    }
}

/// Performs the parse payload operation.
fn parse_payload(line: &str) -> Vec<ClientEvent> {
    if line == "[DONE]" {
        return vec![ClientEvent::Done];
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return Vec::new();
    };
    let mut events = text_events(&value);
    if let Some(usage) = completed_usage(&value) {
        events.push(ClientEvent::Usage(usage));
    }
    if value["type"] == "response.completed" || value["type"] == "response.output_text.done" {
        events.push(ClientEvent::Done);
    }
    if let Some(call) = tool_call(&value) {
        events.push(ClientEvent::ToolCall(call));
    }
    events
}

/// The Responses stream reports final authoritative usage on `response.completed`
/// under `response.usage`. The fallback `/usage` also supports compatible
/// gateways which flatten the completed response.
fn completed_usage(value: &serde_json::Value) -> Option<Usage> {
    let usage = value
        .pointer("/response/usage")
        .or_else(|| value.get("usage"))?;
    Some(Usage {
        input: usage.get("input_tokens")?.as_u64()? as u32,
        output: usage.get("output_tokens")?.as_u64()? as u32,
        cached_input: usage
            .pointer("/input_tokens_details/cached_tokens")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as u32,
    })
}

/// Performs the text events operation.
fn text_events(value: &serde_json::Value) -> Vec<ClientEvent> {
    let mut events = Vec::new();
    if let Some(text) = value["delta"].as_str().filter(|_| {
        value["type"]
            .as_str()
            .is_some_and(|kind| kind.ends_with("output_text.delta"))
    }) {
        events.push(ClientEvent::TextDelta(text.into()));
    }
    if value["type"] == "response.output_text.done" {
        if let Some(text) = value["output_text"].as_str() {
            events.push(ClientEvent::TextDelta(text.into()));
        }
    }
    events
}

/// Performs the tool call operation.
fn tool_call(value: &serde_json::Value) -> Option<ToolCall> {
    if value["type"] != "response.output_item.done"
        || value.pointer("/item/type").and_then(|v| v.as_str()) != Some("function_call")
    {
        return None;
    }
    let (name, arguments, call_id) = (
        value["item"]["name"].as_str()?,
        value["item"]["arguments"].as_str()?,
        value["item"]["call_id"].as_str()?,
    );
    Some(ToolCall {
        call_id: call_id.into(),
        name: name.into(),
        input: serde_json::from_str(arguments).unwrap_or_else(|_| arguments.into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_keepalives_and_waits_for_completion() {
        let input = ": ping\n\ndata: {\"type\":\"response.created\"}\n\ndata: {\"type\":\"response.completed\"}\n";
        assert_eq!(
            read_stream(input.as_bytes()).unwrap(),
            vec![ClientEvent::Done]
        );
    }

    #[test]
    fn classifies_in_stream_failures_by_code() {
        let failed = |code: &str| {
            let event =
                json!({"type":"response.failed","response":{"error":{"code":code,"message":"m"}}});
            read_stream(format!("data: {event}\n").as_bytes()).unwrap_err()
        };
        assert!(failed("server_is_overloaded").retryable());
        assert!(!failed("context_length_exceeded").retryable());
        let cut_short = read_stream("data: {\"type\":\"response.created\"}\n".as_bytes());
        assert!(cut_short.unwrap_err().retryable());
    }
}
