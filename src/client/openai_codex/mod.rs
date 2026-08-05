pub mod auth;

use super::{Client, ClientError, ClientErrorKind, ClientEvent, ClientRequest};
use crate::config::ResolvedConnection;
use crate::model::ToolCall;
use serde_json::json;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::sync::mpsc;
use uuid::Uuid;

pub struct CodexClient {
    connection: ResolvedConnection,
}

fn consume_stream(
    receiver: mpsc::Receiver<Result<String, String>>,
    content_type: String,
    content_length: String,
) -> Result<Vec<ClientEvent>, ClientError> {
    let mut events = Vec::new();
    let mut received_event = false;
    let mut completed = false;
    let mut line_count = 0usize;
    let mut data_line_count = 0usize;
    let mut malformed_data_count = 0usize;
    let mut event_types = BTreeMap::<String, usize>::new();
    loop {
        let Some(line) = receive_line(&receiver)? else {
            break;
        };
        line_count += 1;
        // SSE uses blank lines to delimit events and may use colon-prefixed
        // comments as keepalives. Neither is application data.
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        data_line_count += 1;
        let (done, protocol_event, parsed) =
            decode_data(data.trim(), &mut event_types, &mut malformed_data_count);
        completed |= done;
        apply_events(
            parsed,
            protocol_event,
            &mut events,
            &mut received_event,
            &mut completed,
        );
    }
    finish_stream(
        events,
        completed,
        received_event,
        line_count,
        data_line_count,
        malformed_data_count,
        event_types,
        content_type,
        content_length,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_stream(
    events: Vec<ClientEvent>,
    completed: bool,
    received_event: bool,
    line_count: usize,
    data_line_count: usize,
    malformed_data_count: usize,
    event_types: BTreeMap<String, usize>,
    content_type: String,
    content_length: String,
) -> Result<Vec<ClientEvent>, ClientError> {
    if !completed {
        return Err(ClientError::new(ClientErrorKind::Protocol, format!("Codex stream ended before completion event (received_event={received_event}, parsed_events={}, lines={line_count}, data_lines={data_line_count}, malformed_data={malformed_data_count}, event_types={event_types:?}, content_type={content_type}, content_length={content_length})", events.len())));
    }
    Ok(events)
}

fn receive_line(
    receiver: &mpsc::Receiver<Result<String, String>>,
) -> Result<Option<String>, ClientError> {
    match receiver.recv() {
        Ok(Ok(line)) => Ok(Some(line)),
        Ok(Err(error)) => Err(ClientError::new(
            ClientErrorKind::Transport,
            format!("Codex stream read failed: {error}"),
        )),
        Err(mpsc::RecvError) => Ok(None),
    }
}

fn decode_data(
    data: &str,
    event_types: &mut BTreeMap<String, usize>,
    malformed: &mut usize,
) -> (bool, bool, Vec<ClientEvent>) {
    if data == "[DONE]" {
        return (true, true, vec![ClientEvent::Done]);
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(data) else {
        *malformed += 1;
        return (false, false, Vec::new());
    };
    let kind = value["type"].as_str().unwrap_or("").to_owned();
    *event_types.entry(kind).or_default() += 1;
    (false, true, parse_sse(data))
}

fn apply_events(
    parsed: Vec<ClientEvent>,
    protocol_event: bool,
    events: &mut Vec<ClientEvent>,
    received_event: &mut bool,
    completed: &mut bool,
) {
    if !protocol_event && parsed.is_empty() {
        return;
    }
    // Lifecycle events (for example response.created) are valid stream
    // activity even though they do not produce a ClientEvent. Otherwise a
    // healthy stream can be reported as having received no first event.
    *received_event |= protocol_event;
    *completed |= parsed
        .iter()
        .any(|event| matches!(event, ClientEvent::Done));
    events.extend(parsed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_keepalives_and_waits_for_completion() {
        let (sender, receiver) = mpsc::channel();
        let producer = std::thread::spawn(move || {
            sender.send(Ok(": ping\n".into())).unwrap();
            sender.send(Ok("\n".into())).unwrap();
            sender
                .send(Ok("data: {\"type\":\"response.created\"}\n".into()))
                .unwrap();
            std::thread::sleep(std::time::Duration::from_millis(10));
            sender
                .send(Ok("data: {\"type\":\"response.completed\"}\n".into()))
                .unwrap();
        });

        assert_eq!(
            consume_stream(receiver, "text/event-stream".into(), "missing".into()).unwrap(),
            vec![ClientEvent::Done]
        );
        producer.join().unwrap();
    }
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
        merge_params(&mut body, request.params);
        let url = format!(
            "{}/responses",
            self.connection.connection.base_url.trim_end_matches('/')
        );
        let http = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(15))
            // A streamed Codex turn can legitimately be quiet for more than a
            // few minutes while the model reasons or waits on a tool. Keep the
            // connection timeout, but do not impose a whole-request deadline.
            .timeout(None)
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

        consume_stream(receiver, content_type, content_length)
    }
}

fn merge_params(body: &mut serde_json::Value, params: serde_json::Map<String, serde_json::Value>) {
    if let Some(object) = body.as_object_mut() {
        object.extend(params);
    }
}

pub fn parse_sse(input: &str) -> Vec<ClientEvent> {
    payloads(input)
        .into_iter()
        .flat_map(parse_payload)
        .collect()
}

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

fn parse_payload(line: &str) -> Vec<ClientEvent> {
    if line == "[DONE]" {
        return vec![ClientEvent::Done];
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return Vec::new();
    };
    let mut events = text_events(&value);
    if value["type"] == "response.completed" || value["type"] == "response.output_text.done" {
        events.push(ClientEvent::Done);
    }
    if let Some(call) = tool_call(&value) {
        events.push(ClientEvent::ToolCall(call));
    }
    events
}

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
        input: serde_json::from_str(arguments).ok()?,
    })
}
