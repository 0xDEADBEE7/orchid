//! Provides the command functionality.
use super::{run_with_progress, Provider};
use crate::config::Settings;
use crate::model::{Event, EventKind, Session, Status};
use serde_json::json;
use std::io;

/// Performs the run command operation.
pub fn run_command(
    store: &crate::store::Store,
    settings: &Settings,
    args: &[String],
) -> io::Result<String> {
    let id = command_id(args)?;
    let mut session = load_session(store, settings, &id)?;
    session.metadata.pid = Some(std::process::id());
    let settings = settings.for_session(&session);
    run_session(store, &settings, &id, session)
}

/// Performs the load session operation.
fn load_session(store: &crate::store::Store, settings: &Settings, id: &str) -> io::Result<Session> {
    settings.log(
        id,
        "info",
        "worker started",
        json!({"pid": std::process::id()}),
    );
    store.load(id).inspect_err(|error| {
        let fields = json!({"error": error.to_string()});
        settings.log(id, "error", "worker failed to load session", fields);
    })
}

/// Performs the run session operation.
fn run_session(
    store: &crate::store::Store,
    settings: &Settings,
    id: &str,
    session: Session,
) -> io::Result<String> {
    // Setup errors (missing connection, credential, unsupported interface) end the
    // turn as a recorded failure instead of leaving the session marked running.
    let provider = match configured_provider(settings) {
        Ok(provider) => provider,
        Err(error) => return finish_failure(store, settings, id, session, error),
    };
    run_provider(store, settings, id, session, &provider)
}

/// Performs the run provider operation.
fn run_provider(
    store: &crate::store::Store,
    settings: &Settings,
    id: &str,
    mut session: Session,
    provider: &dyn Provider,
) -> io::Result<String> {
    // Persist each step, then fire on-event for the new events; a hook block
    // (exit 2) is returned to the loop. Queued messages are delivered between
    // requests (their hooks already ran in `send`), and when a turn ends with
    // messages waiting, another turn starts instead of going idle.
    let fired = std::cell::Cell::new(session.events.len());
    loop {
        let result = run_with_progress(
            provider,
            settings,
            &mut session,
            |session| {
                store.save(session).ok()?;
                let blocked = crate::hooks::fire_events(settings, session, fired.get());
                fired.set(session.events.len());
                blocked
            },
            |session| {
                store.drain(session)?;
                fired.set(session.events.len());
                Ok(())
            },
        );
        let reply = match result {
            Ok(reply) => reply,
            Err(error) => {
                return finish_failure(store, settings, id, session, error);
            }
        };
        let last = session.events.len() - 1;
        if !store.finish(&mut session, Status::Idle)? {
            return finish_success(settings, id, session, reply, last);
        }
        fired.set(session.events.len());
    }
}

/// Returns the session id passed to the worker.
fn command_id(args: &[String]) -> io::Result<String> {
    args.windows(2)
        .find(|pair| pair[0] == "--id")
        .map(|pair| pair[1].clone())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "__run requires --id"))
}

/// Performs the configured provider operation.
fn configured_provider(settings: &Settings) -> io::Result<super::ClientProvider> {
    let name = settings.agent.connection.as_str();
    if name.is_empty() {
        let hint = "no connection configured: set the agent's \"connection\"";
        return Err(io::Error::other(hint));
    }
    let resolved = settings
        .resolve_connection(name)
        .map_err(|error| io::Error::new(error.kind(), format!("connection {name}: {error}")))?;
    let client = crate::client::client_for(resolved.clone())
        .map_err(|error| io::Error::other(format!("connection {name}: {error}")))?;
    Ok(super::ClientProvider {
        client,
        model: resolved.connection.model,
        system_prompt: settings.prompt()?,
        tools: settings.agent.tools.clone(),
        params: resolved.params.into_iter().collect(),
    })
}

/// Performs the finish success operation.
fn finish_success(
    settings: &Settings,
    id: &str,
    session: Session,
    reply: String,
    last: usize,
) -> io::Result<String> {
    settings.log(id, "info", "worker finished successfully", json!({}));
    turn_end(settings, &session, last);
    Ok(serde_json::json!({"id":id,"status":"idle","message":reply}).to_string())
}

/// Performs the finish failure operation.
fn finish_failure(
    store: &crate::store::Store,
    settings: &Settings,
    id: &str,
    mut session: Session,
    error: io::Error,
) -> io::Result<String> {
    let message = error.to_string();
    let terminated = message.contains("token threshold") || message.starts_with("blocked by hook");
    session.metadata.termination_reason = Some(message.clone());
    let (kind, status) = if terminated {
        let reason = message.clone();
        (EventKind::Termination { reason }, Status::Terminated)
    } else {
        let message = message.clone();
        (EventKind::Failure { message }, Status::Failed)
    };
    session.append(Event::new(kind));
    let failure = session.events.len() - 1;
    // Messages queued during the failed turn are kept in the transcript (their
    // hooks already ran in `send`); the session is released in the same step.
    store.finish(&mut session, status)?;
    let _ = crate::hooks::fire(settings, "on-event", &session.events[failure], &session);
    turn_end(settings, &session, failure);
    settings.log(id, "error", "worker failed", json!({"error": message}));
    Err(error)
}

/// Fires on-turn-end with the event that ended the turn (the reply or the
/// failure), after success or failure.
fn turn_end(settings: &Settings, session: &Session, index: usize) {
    let _ = crate::hooks::fire(settings, "on-turn-end", &session.events[index], session);
}
