//! Provides the lifecycle functionality.
use super::{dispatch, Provider, StreamEvent};
use crate::{
    client::ClientError,
    config::Settings,
    model::{LogRecord, Session, Usage},
    store::Store,
};
use serde_json::Value;
use std::io;

/// Runs the requested operation.
pub fn run(
    provider: &dyn Provider,
    settings: &Settings,
    session: &mut Session,
) -> io::Result<String> {
    run_with_progress(provider, settings, session, |_| {})
}

/// Performs the run with progress operation.
pub fn run_with_progress<F: FnMut(&Session)>(
    provider: &dyn Provider,
    settings: &Settings,
    session: &mut Session,
    mut progress: F,
) -> io::Result<String> {
    loop {
        enforce_token_limit(session, settings.policy.max_tokens())?;
        progress(session);
        let (answer, usage, calls) = collect(stream_with_retries(provider, settings, session)?);
        let input = match usage.as_ref() {
            Some(tokens) => tokens.input,
            None => estimate_request_tokens(session, &settings.prompt()?),
        };
        session.metadata.token_estimate = input;
        record_usage(session, usage, input);
        progress(session);
        // A reply may carry text and tool calls together (e.g. a preamble before a
        // tool call). Keep the text, then run the calls; only a reply without calls
        // ends the turn.
        if !answer.is_empty() {
            session.append(Session::message("assistant", answer.clone()));
            progress(session);
        }
        if calls.is_empty() {
            return if answer.is_empty() {
                Err(io::Error::other("provider returned no text"))
            } else {
                Ok(answer)
            };
        }
        dispatch::execute(settings, session, calls, &mut progress)?;
    }
}

/// Requests a reply, retrying transient client failures (network errors,
/// overload, 5xx) after 2, 4 and 8 seconds. Nothing is persisted until a reply
/// is complete, so a retry repeats no side effects.
fn stream_with_retries(
    provider: &dyn Provider,
    settings: &Settings,
    session: &Session,
) -> io::Result<Vec<StreamEvent>> {
    let mut delay = 2;
    loop {
        match provider.stream(session) {
            Err(error) if delay <= 8 && retryable(&error) => {
                let fields =
                    serde_json::json!({"error": error.to_string(), "retry_in_seconds": delay});
                let record = LogRecord::new("warning", "transient provider error", fields);
                let _ = Store::new(&settings.root).and_then(|store| {
                    store.log_both(&session.metadata.id, &record, &settings.log_level)
                });
                std::thread::sleep(std::time::Duration::from_secs(delay));
                delay *= 2;
            }
            result => return result,
        }
    }
}

/// Whether an error came from a client failure that may succeed on retry.
fn retryable(error: &io::Error) -> bool {
    error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<ClientError>())
        .is_some_and(ClientError::retryable)
}

/// Estimates input tokens as a quarter of the bytes of the system prompt and the
/// serialized messages sent to clients; used only when the provider reports no usage.
fn estimate_request_tokens(session: &Session, system_prompt: &str) -> u32 {
    let messages: usize = super::messages(session)
        .iter()
        .map(|message| serde_json::to_string(message).map_or(0, |line| line.len() + 1))
        .sum();
    (system_prompt.len() + messages)
        .div_ceil(4)
        .min(u32::MAX as usize) as u32
}

/// Compare the configured limit only with the latest request's input usage.
fn enforce_token_limit(session: &mut Session, limit: Option<i64>) -> io::Result<()> {
    let Some(limit) = limit.filter(|limit| *limit >= 0) else {
        return Ok(());
    };
    let input = session.metadata.token_estimate;
    if i64::from(input) <= limit {
        return Ok(());
    }
    let reason = format!("token threshold exceeded: {input} > {limit}");
    session.metadata.termination_reason = Some(reason.clone());
    Err(io::Error::other(reason))
}
/// Performs the record usage operation.
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
    // Provider usage is authoritative; the local estimate is only a fallback.
    session.metadata.token_usage.marginal_input = input;
    session.metadata.token_usage.cumulative_input += u64::from(input);
    session.metadata.token_usage.cumulative_output += u64::from(output);
    session.metadata.token_usage.cumulative_cached_input += u64::from(cached_input);
    session.metadata.token_usage.requests += 1;
    session.metadata.token_usage.method = method.into();
}
/// Performs the collect operation.
fn collect(events: Vec<StreamEvent>) -> (String, Option<Usage>, Vec<(String, Value)>) {
    events
        .into_iter()
        .fold((String::new(), None, Vec::new()), |mut out, event| {
            match event {
                StreamEvent::Text(text) => out.0.push_str(&text),
                StreamEvent::Usage(tokens) => out.1 = Some(tokens),
                StreamEvent::ToolCall { name, input } => out.2.push((name, input)),
                StreamEvent::Done => {}
            }
            out
        })
}
