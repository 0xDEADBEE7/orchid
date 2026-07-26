use super::{dispatch, Provider, StreamEvent};
use crate::{
    config::Settings,
    model::{Event, Session, Usage},
};
use serde_json::Value;
use std::io;

pub fn run(
    provider: &dyn Provider,
    settings: &Settings,
    session: &mut Session,
    prompt: &str,
) -> io::Result<String> {
    run_with_progress(provider, settings, session, prompt, |_| {})
}

pub fn run_with_progress<F: FnMut(&Session)>(
    provider: &dyn Provider,
    settings: &Settings,
    session: &mut Session,
    prompt: &str,
    mut progress: F,
) -> io::Result<String> {
    for _ in 0..100 {
        let estimated_request_tokens = budget(session, prompt, settings.policy.max_tokens())?;
        progress(session);
        let (answer, usage, calls) = collect(provider.stream(prompt, session)?);
        if malformed(&calls) {
            return Err(io::Error::other("malformed provider content"));
        }
        session.metadata.token_estimate = estimated_request_tokens;
        record_usage(session, usage);
        progress(session);
        if !answer.is_empty() {
            session.append(Session::message("assistant", answer.clone()));
            progress(session);
            return Ok(answer);
        }
        if calls.is_empty() {
            return Err(io::Error::other("provider returned no text"));
        }
        dispatch::execute(settings, session, calls, &mut progress)?;
    }
    Err(io::Error::other("provider tool loop exceeded safety limit"))
}

#[allow(dead_code)]
fn transcript(session: &Session, prompt: &str) -> String {
    let mut context = String::new();
    for event in &session.events {
        match event {
            Event::Message { role, content, .. } => {
                // Roles and separators are protocol overhead, not context
                // content.  The fallback is intentionally only a rough
                // estimate, so count the text sent for each message.
                let _ = role;
                context.push_str(content);
            }
            Event::Reasoning { content, .. } => {
                context.push_str(content);
            }
            Event::ToolCall { calls, .. } => {
                context.push_str(&serde_json::to_string(calls).unwrap_or_default());
            }
            Event::ToolResult {
                call_id: _,
                content,
                ..
            } => {
                context.push_str(&content.to_string());
            }
            Event::Usage { .. } | Event::Termination { .. } | Event::Failure { .. } => {}
        }
    }
    context.push_str(prompt);
    context
}

#[derive(serde::Serialize)]
struct EstimateMessage<'a> {
    role: &'a str,
    content: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<&'a [crate::model::ToolCall]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_result: Option<EstimateToolResult<'a>>,
}

#[derive(serde::Serialize)]
struct EstimateToolResult<'a> {
    call_id: &'a str,
    content: &'a Value,
}

/// Match the historical vendor-agnostic JSON-size estimate: serialized context / 3.
fn estimate_request_tokens(session: &Session, prompt: &str) -> u32 {
    let mut messages = Vec::new();
    for event in &session.events {
        match event {
            Event::Message { role, content, .. } => messages.push(EstimateMessage {
                role,
                content,
                tool_calls: None,
                tool_result: None,
            }),
            Event::ToolCall { calls, .. } => messages.push(EstimateMessage {
                role: "assistant",
                content: "",
                tool_calls: Some(calls),
                tool_result: None,
            }),
            Event::ToolResult {
                call_id, content, ..
            } => messages.push(EstimateMessage {
                role: "user",
                content: "",
                tool_calls: None,
                tool_result: Some(EstimateToolResult { call_id, content }),
            }),
            Event::Reasoning { .. }
            | Event::Usage { .. }
            | Event::Termination { .. }
            | Event::Failure { .. } => {}
        }
    }
    messages.push(EstimateMessage {
        role: "user",
        content: prompt,
        tool_calls: None,
        tool_result: None,
    });
    serde_json::to_string(&messages)
        .map(|serialized| (serialized.len() / 3) as u32)
        .unwrap_or(0)
}

fn malformed(calls: &[(String, Value)]) -> bool {
    calls
        .first()
        .is_some_and(|(name, _)| name == "__malformed__")
}
fn budget(session: &mut Session, pending: &str, limit: i64) -> io::Result<u32> {
    let estimate = estimate_request_tokens(session, pending);
    if limit == -1 {
        return Ok(estimate);
    }
    if i64::from(estimate) > limit {
        session.metadata.termination_reason =
            Some("token threshold exceeded before provider request".into());
        Err(io::Error::other("token threshold exceeded"))
    } else {
        Ok(estimate)
    }
}
fn record_usage(session: &mut Session, usage: Option<Usage>) {
    let (input, output) = usage
        .as_ref()
        .map(|tokens| (tokens.input, tokens.output))
        .unwrap_or((0, 0));
    if usage.is_some() {
        session.append(Event::Usage {
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            input,
            output,
        });
    }
}
fn collect(events: Vec<StreamEvent>) -> (String, Option<Usage>, Vec<(String, Value)>) {
    events
        .into_iter()
        .fold((String::new(), None, Vec::new()), |mut out, event| {
            match event {
                StreamEvent::Text(text) => out.0.push_str(&text),
                StreamEvent::Usage(tokens) => out.1 = Some(tokens),
                StreamEvent::ToolCall { name, input } => out.2.push((name, input)),
                StreamEvent::Done => {}
                StreamEvent::Malformed(message) => {
                    out.2.push(("__malformed__".into(), Value::String(message)))
                }
            }
            out
        })
}
