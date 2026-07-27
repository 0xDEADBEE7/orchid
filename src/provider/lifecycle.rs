use super::{dispatch, Provider, StreamEvent};
use crate::{
    config::Settings,
    model::{Event, Session, Usage},
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
    for _ in 0..100 {
        let estimated_request_tokens = budget(session, prompt, settings.policy.max_tokens())?;
        progress(session);
        let (answer, usage, calls) = collect(provider.stream(prompt, session)?);
        if malformed(&calls) {
            return Err(io::Error::other("malformed provider content"));
        }
        session.metadata.token_estimate = estimated_request_tokens;
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
    Err(io::Error::other("provider tool loop exceeded safety limit"))
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
    for event in &session.events {
        let Ok(line) = serde_json::to_string(event) else {
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
fn record_usage(session: &mut Session, usage: Option<Usage>, estimate: u32) {
    let (input, output, method) = usage
        .as_ref()
        .map(|tokens| (tokens.input, tokens.output, "provider_reported"))
        .unwrap_or((estimate, 0, "local_tokenizer"));
    session.metadata.token_usage.context_estimate = estimate;
    session.metadata.token_usage.input_total += u64::from(input);
    session.metadata.token_usage.output_total += u64::from(output);
    session.metadata.token_usage.requests += 1;
    session.metadata.token_usage.method = method.into();
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
