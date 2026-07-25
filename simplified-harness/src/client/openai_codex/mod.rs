pub mod auth;

use super::{Client, ClientError, ClientErrorKind, ClientEvent, ClientRequest};
use crate::config::ResolvedConnection;
use crate::model::ToolCall;
use serde_json::json;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use uuid::Uuid;

const FIRST_EVENT_TIMEOUT: Duration = Duration::from_secs(30);
const STREAM_INACTIVITY_TIMEOUT: Duration = Duration::from_secs(30);

pub struct CodexClient {
    connection: ResolvedConnection,
}
impl CodexClient {
    pub fn new(connection: ResolvedConnection) -> Self {
        Self { connection }
    }
}
impl Client for CodexClient {
    fn stream(&self, request: ClientRequest) -> Result<Vec<ClientEvent>, ClientError> {
        let credential = self.connection.credential.as_ref().ok_or_else(|| {
            ClientError::new(
                ClientErrorKind::Authentication,
                "Codex credential unavailable",
            )
        })?;
        let (token, account) = auth::CodexAuth::present(credential)?;
        let mut body = json!({
            "model": request.model,
            "instructions": request.system_prompt,
            "input": request.messages.iter().flat_map(|m| {
                if !m.tool_calls.is_empty() {
                    return m.tool_calls.iter().map(|call| json!({"type":"function_call","call_id":call.call_id,"name":call.name,"arguments":call.input.to_string()})).collect::<Vec<_>>();
                }
                if let Some((call_id, content)) = &m.tool_result {
                    return vec![json!({"type":"function_call_output","call_id":call_id,"output":content.to_string()})];
                }
                vec![json!({"role":m.role,"content":[{"type":if m.role == "assistant" {"output_text"} else {"input_text"},"text":m.content}]})]
            }).collect::<Vec<_>>(),
            "tools": request.tools.iter().map(|tool| json!({"type":"function","name":tool.name,"description":tool.description,"parameters":tool.parameters})).collect::<Vec<_>>(),
            "store": false,
            "stream": true
        });
        if let Some(object) = body.as_object_mut() {
            for (key, value) in request.params {
                object.insert(key, value);
            }
        }
        let url = format!(
            "{}/responses",
            self.connection.connection.base_url.trim_end_matches('/')
        );
        let http = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(15))
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| ClientError::new(ClientErrorKind::Transport, e.to_string()))?;
        let mut call = http
            .post(url)
            .bearer_auth(token)
            .header("ChatGPT-Account-ID", account)
            .header("originator", "codex_cli_rs")
            .header("openai-beta", "responses=experimental")
            .header("Version", "0.144.4")
            .header("Session_Id", Uuid::new_v4().to_string())
            .header("User-Agent", "codex_cli_rs/0.144.4 (orchid)")
            .header("Accept", "text/event-stream")
            .header("Content-Type", "application/json")
            .json(&body);
        for (name, value) in &self.connection.headers {
            call = call.header(name, value);
        }
        let response = call.send().map_err(|e| {
            ClientError::new(
                ClientErrorKind::Transport,
                format!("Codex request dispatch failed: {e}"),
            )
        })?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("missing")
            .to_owned();
        let content_length = response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("missing")
            .to_owned();
        if !status.is_success() {
            return Err(ClientError::new(
                if status.as_u16() == 401 || status.as_u16() == 403 {
                    ClientErrorKind::Authentication
                } else {
                    ClientErrorKind::Http
                },
                format!("Codex HTTP response status {}", status.as_u16()),
            ));
        }

        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(response);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) if sender.send(Ok(line.clone())).is_err() => break,
                    Err(error) => {
                        let _ = sender.send(Err(error.to_string()));
                        break;
                    }
                    _ => {}
                }
            }
        });

        let mut events = Vec::new();
        let mut received_event = false;
        let mut completed = false;
        let mut line_count = 0usize;
        let mut data_line_count = 0usize;
        let mut malformed_data_count = 0usize;
        let mut event_types = BTreeMap::<String, usize>::new();
        let mut deadline = Instant::now() + FIRST_EVENT_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                let phase = if received_event {
                    "stream inactivity"
                } else {
                    "first event"
                };
                return Err(ClientError::new(
                    ClientErrorKind::Transport,
                    format!(
                        "Codex {phase} timeout after {}s",
                        if received_event {
                            STREAM_INACTIVITY_TIMEOUT.as_secs()
                        } else {
                            FIRST_EVENT_TIMEOUT.as_secs()
                        }
                    ),
                ));
            }
            match receiver.recv_timeout(remaining) {
                Ok(Ok(line)) => {
                    line_count += 1;
                    if let Some(data) = line.strip_prefix("data: ") {
                        data_line_count += 1;
                        let data = data.trim();
                        if data == "[DONE]" {
                            events.push(ClientEvent::Done);
                            completed = true;
                        } else {
                            match serde_json::from_str::<serde_json::Value>(data) {
                                Ok(value) => {
                                    if let Some(kind) = value["type"].as_str() {
                                        *event_types.entry(kind.to_owned()).or_default() += 1;
                                    }
                                    let parsed = parse_sse(data);
                                    if !parsed.is_empty() {
                                        received_event = true;
                                        deadline = Instant::now() + STREAM_INACTIVITY_TIMEOUT;
                                        if parsed.iter().any(|event| matches!(event, ClientEvent::Done)) {
                                            completed = true;
                                        }
                                        events.extend(parsed);
                                    }
                                }
                                Err(_) => malformed_data_count += 1,
                            }
                        }
                    }
                }
                Ok(Err(error)) => {
                    return Err(ClientError::new(
                        ClientErrorKind::Transport,
                        format!("Codex stream read failed: {error}"),
                    ))
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let phase = if received_event {
                        "stream inactivity"
                    } else {
                        "first event"
                    };
                    return Err(ClientError::new(
                        ClientErrorKind::Transport,
                        format!(
                            "Codex {phase} timeout after {}s",
                            if received_event {
                                STREAM_INACTIVITY_TIMEOUT.as_secs()
                            } else {
                                FIRST_EVENT_TIMEOUT.as_secs()
                            }
                        ),
                    ));
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        if !completed {
            return Err(ClientError::new(
                ClientErrorKind::Protocol,
                format!(
                    "Codex stream ended before completion event (received_event={}, parsed_events={}, lines={}, data_lines={}, malformed_data={}, event_types={:?}, content_type={}, content_length={})",
                    received_event,
                    events.len(),
                    line_count,
                    data_line_count,
                    malformed_data_count,
                    event_types,
                    content_type,
                    content_length
                ),
            ));
        }
        Ok(events)
    }
}

pub fn parse_sse(input: &str) -> Vec<ClientEvent> {
    let mut events = Vec::new();
    let payloads = if input.trim_start().starts_with('{') {
        vec![input.trim()]
    } else {
        input
            .lines()
            .filter_map(|line| line.strip_prefix("data:").map(str::trim))
            .collect()
    };
    for line in payloads {
        if line == "[DONE]" {
            events.push(ClientEvent::Done);
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if let Some(text) = value["delta"].as_str().filter(|_| {
            value["type"]
                .as_str()
                .is_some_and(|kind| kind.ends_with("output_text.delta"))
        }) {
            events.push(ClientEvent::TextDelta(text.into()));
        }
        if value["type"] == "response.output_text.done"
            && value["output_text"].as_str().is_some()
        {
            events.push(ClientEvent::TextDelta(
                value["output_text"].as_str().unwrap_or_default().into(),
            ));
        }
        if value["type"] == "response.completed"
            || value["type"] == "response.output_text.done"
        {
            events.push(ClientEvent::Done);
        }
        if value["type"] == "response.output_item.done"
            && value.pointer("/item/type").and_then(|v| v.as_str()) == Some("function_call")
        {
            if let (Some(name), Some(arguments), Some(call_id)) = (
                value["item"]["name"].as_str(),
                value["item"]["arguments"].as_str(),
                value["item"]["call_id"].as_str(),
            ) {
                if let Ok(input) = serde_json::from_str(arguments) {
                    events.push(ClientEvent::ToolCall(ToolCall {
                        call_id: call_id.into(),
                        name: name.into(),
                        input,
                    }));
                }
            }
        }
    }
    events
}
