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
        record_usage(session, usage, estimated_request_tokens);
        progress(session);
        if !answer.is_empty() {
            session.append(Session::message("assistant", answer.clone()));
            return Ok(answer);
        }
        if calls.is_empty() {
            return Err(io::Error::other("provider returned no text"));
        }
        dispatch::execute(settings, session, calls)?;
        progress(session);
    }
    Err(io::Error::other("provider tool loop exceeded safety limit"))
}

#[allow(dead_code)]
fn transcript(session: &Session, prompt: &str) -> String {
    let mut context = String::new();
    for event in &session.events {
        match event {
            Event::Message { role, content, .. } => {
                context.push_str(role);
                context.push_str(": ");
                context.push_str(content);
                context.push('\n');
            }
            Event::Reasoning { content, .. } => {
                context.push_str("reasoning: ");
                context.push_str(content);
                context.push('\n');
            }
            Event::ToolCall { calls, .. } => {
                context.push_str("assistant tool call: ");
                context.push_str(&serde_json::to_string(calls).unwrap_or_default());
                context.push('\n');
            }
            Event::ToolResult {
                call_id, content, ..
            } => {
                context.push_str("tool result ");
                context.push_str(call_id);
                context.push_str(": ");
                context.push_str(&content.to_string());
                context.push('\n');
            }
            Event::Usage { .. } | Event::Termination { .. } | Event::Failure { .. } => {}
        }
    }
    if context.is_empty() {
        prompt.to_owned()
    } else {
        format!("{context}\ncurrent user request: {prompt}")
    }
}

fn malformed(calls: &[(String, Value)]) -> bool {
    calls
        .first()
        .is_some_and(|(name, _)| name == "__malformed__")
}
fn budget(session: &mut Session, pending: &str, limit: u32) -> io::Result<u32> {
    let estimate = transcript(session, pending).chars().count().div_ceil(3) as u32;
    if session.state.token_estimate.saturating_add(estimate) > limit {
        session.state.termination_reason =
            Some("token threshold exceeded before provider request".into());
        Err(io::Error::other("token threshold exceeded"))
    } else {
        Ok(estimate)
    }
}
fn record_usage(session: &mut Session, usage: Option<Usage>, fallback: u32) {
    let (input, output, total) = if let Some(ref tokens) = usage {
        (tokens.input, tokens.output, tokens.input + tokens.output)
    } else {
        (fallback, 0, fallback)
    };
    session.state.token_estimate += total;
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
