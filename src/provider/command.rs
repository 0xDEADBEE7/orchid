use super::{run_with_progress, Provider};
use crate::config::Settings;
use crate::model::Session;
use std::io;

pub fn run_command(
    store: &crate::store::Store,
    settings: &Settings,
    args: &[String],
) -> io::Result<String> {
    let id = args
        .windows(2)
        .find(|pair| pair[0] == "--id")
        .map(|pair| pair[1].clone())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "__run requires --id"))?;
    let message = args
        .iter()
        .rev()
        .find(|arg| !arg.starts_with('-') && *arg != &id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "__run requires a message"))?
        .clone();
    let record = || crate::model::LogRecord {
        event_id: uuid::Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now(),
        level: "info".into(),
        message: "worker started".into(),
        fields: serde_json::json!({"pid":std::process::id(),"session_id":id}),
    };
    let _ = store.log_both(&id, &record(), &settings.log_level);
    log(store, settings, &id, "debug", "worker loading session");
    let mut session = match store.load(&id) {
        Ok(session) => session,
        Err(error) => {
            let mut failure = record();
            failure.level = "error".into();
            failure.message = "worker failed to load session".into();
            failure.fields = serde_json::json!({"error":error.to_string()});
            let _ = store.log_both(&id, &failure, &settings.log_level);
            return Err(error);
        }
    };
    let settings = settings.resolve_agent(&session.state.agent)?;
    let configured = settings
        .policy
        .connections
        .first()
        .map(|name| {
            let resolved = settings.resolve_connection(name)?;
            let client = crate::client::client_for(resolved.clone())
                .map_err(|error| io::Error::other(error.to_string()))?;
            Ok::<super::ClientProvider, io::Error>(super::ClientProvider {
                client,
                model: resolved.connection.model,
                system_prompt: settings.prompt()?,
                tools: settings.policy.tools().to_vec(),
                params: resolved.params.into_iter().collect(),
            })
        })
        .transpose()?;
    if !settings.policy.connections.is_empty() && configured.is_none() {
        return finish_failure(
            store,
            &settings,
            &id,
            session,
            io::Error::other("configured connection unavailable"),
        );
    }
    let local = super::LocalProvider;
    let provider: &dyn Provider = configured.as_ref().map_or(&local, |provider| provider);
    log(
        store,
        &settings,
        &id,
        "debug",
        if configured.is_some() {
            "client selected"
        } else {
            "local client selected"
        },
    );
    log(store, &settings, &id, "debug", "client request prepared");
    log(store, &settings, &id, "debug", "client run started");
    log(store, &settings, &id, "debug", "client request dispatched");
    let _ = crate::hooks::run(&settings, "on-turn-start", &id);
    let mut dispatched = session.events.len();
    match run_with_progress(provider, &settings, &mut session, &message, |session| {
        if store.save(session).is_ok() {
            let _ = crate::hooks::dispatch_events(&settings, session, dispatched);
            dispatched = session.events.len();
        }
    }) {
        Ok(reply) => {
            log(store, &settings, &id, "debug", "client stream completed");
            finish_success(store, &settings, &id, session, reply)
        }
        Err(error) => {
            log(
                store,
                &settings,
                &id,
                "error",
                &format!("client stream failed: {error}"),
            );
            finish_failure(store, &settings, &id, session, error)
        }
    }
}

fn finish_success(
    store: &crate::store::Store,
    settings: &Settings,
    id: &str,
    mut session: Session,
    reply: String,
) -> io::Result<String> {
    if store.load(id)?.state.status != crate::model::Status::Cancelled {
        session.state.status = crate::model::Status::Idle;
        session.state.pid = None;
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

fn finish_failure(
    store: &crate::store::Store,
    settings: &Settings,
    id: &str,
    mut session: Session,
    error: io::Error,
) -> io::Result<String> {
    let message = error.to_string();
    let terminated = message.contains("token threshold");
    session.state.status = if terminated {
        crate::model::Status::Terminated
    } else {
        crate::model::Status::Failed
    };
    session.state.pid = None;
    session.state.termination_reason = Some(message.clone());
    if terminated {
        session.append(crate::model::Event::Termination {
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            reason: message.clone(),
        });
    } else {
        session.append(crate::model::Event::Failure {
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            message: message.clone(),
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
    let _ = crate::hooks::run(settings, "on-error", id);
    Err(error)
}

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
