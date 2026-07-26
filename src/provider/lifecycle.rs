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
        session.state.token_estimate = estimated_request_tokens;
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

/// Estimate the size of the serialized message history sent to the provider.
/// This is a snapshot of the current context, not cumulative provider usage.
fn estimate_request_tokens(session: &Session, prompt: &str) -> u32 {
    let mut messages = session
        .events
        .iter()
        .filter_map(|event| match event {
            Event::Message { role, content, .. } => Some(crate::client::Message {
                role: role.clone(),
                content: content.clone(),
                tool_calls: Vec::new(),
                tool_result: None,
            }),
            Event::ToolCall { calls, .. } => Some(crate::client::Message {
                role: "assistant".into(),
                content: String::new(),
                tool_calls: calls.clone(),
                tool_result: None,
            }),
            Event::ToolResult {
                call_id, content, ..
            } => Some(crate::client::Message {
                role: "tool".into(),
                content: content.to_string(),
                tool_calls: Vec::new(),
                tool_result: Some((call_id.clone(), content.clone())),
            }),
            Event::Reasoning { .. }
            | Event::Usage { .. }
            | Event::Termination { .. }
            | Event::Failure { .. } => None,
        })
        .collect::<Vec<_>>();
    messages.push(crate::client::Message {
        role: "user".into(),
        content: prompt.into(),
        tool_calls: Vec::new(),
        tool_result: None,
    });
    let bytes = serde_json::to_string(&messages)
        .map(|s| s.len())
        .unwrap_or(0);
    (bytes / 3) as u32
}

fn malformed(calls: &[(String, Value)]) -> bool {
    calls
        .first()
        .is_some_and(|(name, _)| name == "__malformed__")
}
fn budget(session: &mut Session, pending: &str, limit: u32) -> io::Result<u32> {
    let estimate = estimate_request_tokens(session, pending);
    if estimate > limit {
        session.state.termination_reason =
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
