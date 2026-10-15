//! Provides the command functionality.
use super::{run_with_progress, Provider};
use crate::config::Settings;
use crate::model::Session;
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
    let record = crate::model::LogRecord::new(
        "info",
        "worker started",
        serde_json::json!({"pid":std::process::id(),"session_id":id}),
    );
    let _ = store.log_both(id, &record, &settings.log_level);
    log(store, settings, id, "debug", "worker loading session");
    store.load(id).inspect_err(|error| {
        let record = crate::model::LogRecord::new(
            "error",
            "worker failed to load session",
            serde_json::json!({"error":error.to_string()}),
        );
        let _ = store.log_both(id, &record, &settings.log_level);
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
    log(store, settings, id, "debug", "client selected");
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
                let message = format!("client stream failed: {error}");
                log(store, settings, id, "error", &message);
                return finish_failure(store, settings, id, session, error);
            }
        };
        if !store.finish(&mut session)? {
            return finish_success(store, settings, id, session, reply);
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
    store: &crate::store::Store,
    settings: &Settings,
    id: &str,
    session: Session,
    reply: String,
) -> io::Result<String> {
    log(store, settings, id, "info", "worker finished successfully");
    turn_end(settings, &session);
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
    session.metadata.status = if terminated {
        crate::model::Status::Terminated
    } else {
        crate::model::Status::Failed
    };
    session.metadata.pid = None;
    session.metadata.termination_reason = Some(message.clone());
    if terminated {
        session.append(crate::model::Event::Termination {
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            reason: message.clone(),
            connection: None,
            token_usage: crate::model::TokenUsage::default(),
        });
    } else {
        session.append(crate::model::Event::Failure {
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            message: message.clone(),
            connection: None,
            token_usage: crate::model::TokenUsage::default(),
        });
    }
    store.save(&session)?;
    let _ = crate::hooks::fire_events(settings, &session, session.events.len() - 1);
    turn_end(settings, &session);
    let _ = store.log_both(
        id,
        &crate::model::LogRecord::new(
            "error",
            "worker failed",
            serde_json::json!({"error":message}),
        ),
        &settings.log_level,
    );
    log(store, settings, id, "error", &message);
    Err(error)
}

/// Fires on-turn-end with the turn's last event, after success or failure.
fn turn_end(settings: &Settings, session: &Session) {
    if let Some(last) = session.events.last() {
        let _ = crate::hooks::fire(settings, "on-turn-end", last, session);
    }
}

/// Records a lifecycle message.
fn log(store: &crate::store::Store, settings: &Settings, id: &str, level: &str, message: &str) {
    let _ = store.log_both(
        id,
        &crate::model::LogRecord::new(level, message, serde_json::json!({})),
        &settings.log_level,
    );
}
