use super::{dispatch, Provider, StreamEvent};
use crate::{
    config::Settings,
    model::{Session, Usage},
};
use serde_json::Value;
use std::io;
use tiktoken_rs::o200k_base;

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
    loop {
        let estimated_request_tokens = budget(session, prompt, settings.policy.max_tokens())?;
        progress(session);
        let (answer, usage, calls) = collect(provider.stream(prompt, session)?);
        if malformed(&calls) {
            return Err(io::Error::other("malformed provider content"));
        }
        session.metadata.token_estimate = usage
            .as_ref()
            .map_or(estimated_request_tokens, |tokens| tokens.input);
        record_usage(session, usage, estimated_request_tokens);
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
}

#[derive(serde::Serialize)]
struct PendingMessage<'a> {
    #[serde(rename = "type")]
    event_type: &'static str,
    role: &'static str,
    content: &'a str,
}

/// Count the serialized JSONL transcript, including one newline per event.
fn estimate_request_tokens(session: &Session, prompt: &str) -> u32 {
    let tokenizer = match o200k_base() {
        Ok(tokenizer) => tokenizer,
        Err(_) => return 0,
    };
    let mut tokens = 0usize;
    // Estimate the same filtered transcript sent to clients. Internal event
    // metadata (timestamps, IDs, token usage, and usage-only events) is not
    // conversation context and must not inflate the fallback estimate.
    for message in super::messages(session) {
        let Ok(line) = serde_json::to_string(&message) else {
            return 0;
        };
        tokens += tokenizer.encode_ordinary(&format!("{line}\n")).len();
    }
    let pending = PendingMessage {
        event_type: "message",
        role: "user",
        content: prompt,
    };
    let Ok(line) = serde_json::to_string(&pending) else {
        return 0;
    };
    tokens += tokenizer.encode_ordinary(&format!("{line}\n")).len();
    tokens.min(u32::MAX as usize) as u32
}

fn malformed(calls: &[(String, Value)]) -> bool {
    calls
        .first()
        .is_some_and(|(name, _)| name == "__malformed__")
}
fn budget(session: &mut Session, pending: &str, limit: Option<i64>) -> io::Result<u32> {
    let estimate = estimate_request_tokens(session, pending);
    let Some(limit) = limit.filter(|limit| *limit != -1) else {
        return Ok(estimate);
    };
    if i64::from(estimate) > limit {
        session.metadata.termination_reason =
            Some("token threshold exceeded before provider request".into());
        Err(io::Error::other("token threshold exceeded"))
    } else {
        Ok(estimate)
    }
}
fn record_usage(session: &mut Session, usage: Option<Usage>, estimate: u32) {
    let (input, output, cached_input, method) = usage
        .as_ref()
        .map(|tokens| {
            (
                tokens.input,
                tokens.output,
                tokens.cached_input,
                "provider_reported",
            )
        })
        .unwrap_or((estimate, 0, 0, "local_tokenizer"));
    // Keep the local estimate separately in `metadata.token_estimate`; when a
    // provider reports usage, the per-request token-usage value is authoritative.
    session.metadata.token_usage.marginal_input = input;
    session.metadata.token_usage.cumulative_input += u64::from(input);
    session.metadata.token_usage.cumulative_output += u64::from(output);
    session.metadata.token_usage.cumulative_cached_input += u64::from(cached_input);
    session.metadata.token_usage.requests += 1;
    session.metadata.token_usage.method = method.into();
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
