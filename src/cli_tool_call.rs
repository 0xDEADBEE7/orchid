//! Provides the cli tool call functionality.
use orchid::{
    config::Settings,
    model::{Session, Status},
    provider::dispatch,
    store::Store,
};
use serde_json::Value;
use std::{env, io};

/// Performs the tool call operation.
pub fn tool_call(store: &Store, settings: &Settings, args: &[String]) -> io::Result<String> {
    let id = value(args, "--id").ok_or_else(|| invalid("tool-call requires --id"))?;
    let raw = value(args, "--input").ok_or_else(|| invalid("tool-call requires --input"))?;
    let request: Value = serde_json::from_str(&raw)
        .map_err(|error| invalid(&format!("invalid --input JSON: {error}")))?;
    let name = request
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("tool-call input requires string name"))?;
    let input = request
        .get("input")
        .filter(|value| value.is_object())
        .cloned()
        .ok_or_else(|| invalid("tool-call input requires object input"))?;
    let call_id = request
        .get("call_id")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let mut session = store.load(&id)?;
    authorize(&session, &id)?;
    if call_id.as_ref().is_some_and(|id| session.events.iter().any(|event| {
        matches!(event, orchid::model::Event::ToolCall { calls, .. } if calls.iter().any(|call| &call.call_id == id))
    })) {
        return Err(invalid("tool-call call_id already exists"));
    }
    let before = session.events.len();
    let result = dispatch::execute_with_ids(
        settings,
        &mut session,
        vec![(name.to_owned(), input, call_id)],
        |_| {},
    );
    store.save(&session)?;
    orchid::hooks::dispatch_events(settings, &session, before)?;
    result.map(|_| String::new())
}

/// Performs the authorize operation.
fn authorize(session: &Session, id: &str) -> io::Result<()> {
    match session.metadata.status {
        Status::Idle | Status::Failed => Ok(()),
        Status::HookRunning => {
            let expected = env::var("ORCHID_HOOK_TOKEN").ok();
            let token_path = std::env::var("ORCHID_HOOK_TOKEN_FILE").ok();
            let valid = expected.zip(token_path).is_some_and(|(token, path)| {
                std::fs::read_to_string(path)
                    .map(|stored| stored.trim() == token)
                    .unwrap_or(false)
            });
            if valid {
                Ok(())
            } else {
                Err(invalid(&format!(
                    "tool-call is not authorized for session {id}"
                )))
            }
        }
        Status::Running => Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "session is already running",
        )),
        Status::Cancelled | Status::Terminated => {
            Err(invalid("session is not available for tool-call"))
        }
    }
}

/// Returns the named argument value, if present.
fn value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
        .or_else(|| {
            args.iter()
                .find_map(|arg| arg.strip_prefix(&format!("{name}=")).map(str::to_owned))
        })
}

/// Creates an invalid-input error.
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
