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
    let settings = settings.settings_for_session(&id, &session.metadata.policy);
    run_session(store, &settings, &id, session)
}

/// Performs the load session operation.
fn load_session(store: &crate::store::Store, settings: &Settings, id: &str) -> io::Result<Session> {
    let record = worker_record(
        "info",
        "worker started",
        serde_json::json!({"pid":std::process::id(),"session_id":id}),
    );
    let _ = store.log_both(id, &record, &settings.log_level);
    log(store, settings, id, "debug", "worker loading session");
    store.load(id).inspect_err(|error| {
        let record = worker_record(
            "error",
            "worker failed to load session",
            serde_json::json!({"error":error.to_string()}),
        );
        let _ = store.log_both(id, &record, &settings.log_level);
    })
}

/// Performs the worker record operation.
fn worker_record(level: &str, message: &str, fields: serde_json::Value) -> crate::model::LogRecord {
    crate::model::LogRecord {
        event_id: uuid::Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now(),
        level: level.into(),
        message: message.into(),
        fields,
    }
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
    let _ = crate::hooks::run(settings, "on-turn-start", id);
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
    let mut dispatched = session.events.len();
    match run_with_progress(provider, settings, &mut session, |session| {
        if store.save(session).is_ok() {
            let _ = crate::hooks::dispatch_events(settings, session, dispatched);
            dispatched = session.events.len();
        }
    }) {
        Ok(reply) => {
            log(store, settings, id, "debug", "client stream completed");
            finish_success(store, settings, id, session, reply)
        }
        Err(error) => {
            log(
                store,
                settings,
                id,
                "error",
                &format!("client stream failed: {error}"),
            );
            finish_failure(store, settings, id, session, error)
        }
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
    let name = settings.active_connection()?.ok_or_else(|| {
        io::Error::other(
            "no connection configured: add connections/<name>.json and list it in the policy",
        )
    })?;
    let resolved = settings
        .resolve_connection(name)
        .map_err(|error| io::Error::new(error.kind(), format!("connection {name}: {error}")))?;
    let client = crate::client::client_for(resolved.clone())
        .map_err(|error| io::Error::other(format!("connection {name}: {error}")))?;
    Ok(super::ClientProvider {
        client,
        model: resolved.connection.model,
        system_prompt: settings.prompt()?,
        tools: settings.policy.tools().to_vec(),
        params: resolved.params.into_iter().collect(),
    })
}

/// Performs the finish success operation.
fn finish_success(
    store: &crate::store::Store,
    settings: &Settings,
    id: &str,
    mut session: Session,
    reply: String,
) -> io::Result<String> {
    if store.load(id)?.metadata.status != crate::model::Status::Cancelled {
        session.metadata.status = crate::model::Status::Idle;
        session.metadata.pid = None;
        store.save(&session)?;
    }
    let _ = store.log_both(
        id,
        &crate::model::LogRecord {
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            level: "info".into(),
            message: "worker finished successfully".into(),
            fields: serde_json::json!({}),
        },
        &settings.log_level,
    );
    log(store, settings, id, "info", "run completed");
    let _ = crate::hooks::run(settings, "on-turn-end", id);
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
    let terminated = message.contains("token threshold");
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
    let _ =
        crate::hooks::dispatch_events(settings, &session, session.events.len().saturating_sub(1));
    let _ = store.log_both(
        id,
        &crate::model::LogRecord {
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            level: "error".into(),
            message: "worker failed".into(),
            fields: serde_json::json!({"error":message}),
        },
        &settings.log_level,
    );
    log(store, settings, id, "error", &message);
    Err(error)
}

/// Records a lifecycle message.
fn log(store: &crate::store::Store, settings: &Settings, id: &str, level: &str, message: &str) {
    let _ = store.log_both(
        id,
        &crate::model::LogRecord {
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            level: level.into(),
            message: message.into(),
            fields: serde_json::json!({}),
        },
        &settings.log_level,
    );
}
