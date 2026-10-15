//! Provides the cli send functionality.
use orchid::{
    config::Settings,
    model::{Event, Session, Status},
    store::Store,
};
use std::{
    env,
    fs::OpenOptions,
    io,
    process::{Command, Stdio},
};

/// Performs the send operation.
pub fn send(store: &Store, settings: &Settings, args: &[String]) -> io::Result<String> {
    let (id, message) = request(args)?;
    run_send(
        store,
        settings,
        &id,
        &message,
        args.iter().any(|arg| arg == "--no-run"),
    )
}

/// Performs the run send operation.
fn run_send(
    store: &Store,
    settings: &Settings,
    id: &str,
    message: &str,
    no_run: bool,
) -> io::Result<String> {
    // Reconciling first means a dead worker's stale `running` status never blocks.
    let session = store.reconcile(id)?;
    let settings = &settings.for_session(&session);
    // Hooks for a user message run before it is stored or queued, so a block
    // (exit 2) keeps it out of the transcript.
    let event = Session::message("user", message.to_owned());
    let mut preview = session.clone();
    preview.append(event.clone());
    if let Some(reason) = orchid::hooks::fire_events(settings, &preview, session.events.len()) {
        let reason = format!("blocked by hook: {reason}");
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, reason));
    }
    let event_id = match &event {
        Event::Message { event_id, .. } => event_id.clone(),
        _ => String::new(),
    };
    // While a worker runs, the message is queued and delivered before its next request.
    let Some(session) = store.deliver(id, event, !no_run)? else {
        return Ok(serde_json::json!({"id":id,"status":"queued","event_id":event_id}).to_string());
    };
    if no_run {
        return Ok(
            serde_json::json!({"id":id,"status":session.metadata.status,"no_run":true}).to_string(),
        );
    }
    start_worker(store, settings, id)
}

/// Performs the start worker operation.
fn start_worker(store: &Store, settings: &Settings, id: &str) -> io::Result<String> {
    log(store, settings, id, "info", "send accepted");
    let child = match spawn_worker(settings, id) {
        Ok(child) => child,
        Err(error) => {
            log(
                store,
                settings,
                id,
                "error",
                &format!("worker spawn failed: {error}"),
            );
            store.update(id, |session| {
                session.metadata.status = Status::Idle;
                session.metadata.pid = None;
            })?;
            return Err(error);
        }
    };
    store.update(id, |session| session.metadata.pid = Some(child.id()))?;
    log(
        store,
        settings,
        id,
        "info",
        &format!("worker spawned: {}", child.id()),
    );
    Ok(serde_json::json!({"id":id,"status":Status::Running,"pid":child.id()}).to_string())
}

/// Builds and validates a request.
fn request(args: &[String]) -> io::Result<(String, String)> {
    if args.iter().any(|arg| arg == "--await") {
        return Err(invalid("send does not support --await; use await <ID>"));
    }
    let id = value(args, "--id").ok_or_else(|| invalid("send requires --id"))?;
    let message = positional(args)
        .first()
        .ok_or_else(|| invalid("send requires a message"))?
        .clone();
    Ok((id, message))
}

/// Records a lifecycle message.
fn log(store: &Store, settings: &Settings, id: &str, level: &str, message: &str) {
    let _ = store.log_both(
        id,
        &orchid::model::LogRecord::new(level, message, serde_json::json!({})),
        &settings.log_level,
    );
}

/// Performs the spawn worker operation.
fn spawn_worker(settings: &Settings, id: &str) -> io::Result<std::process::Child> {
    let stderr_path = settings
        .root
        .join("sessions")
        .join(id)
        .join("worker.stderr.log");
    let stderr = OpenOptions::new()
        .create(true)
        .append(true)
        .open(stderr_path)?;
    Command::new(env::current_exe()?)
        .arg("--config")
        .arg(&settings.root)
        .arg("__run")
        .arg("--id")
        .arg(id)
        // A worker's turn is a fresh root for hook nesting.
        .env_remove("ORCHID_HOOK_DEPTH")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr)
        .spawn()
}

/// Performs the positional operation.
fn positional(args: &[String]) -> Vec<String> {
    let flags = [
        "--id",
        "--label",
        "--working-dir",
        "--agent",
        "--timeout",
        "--interval",
    ];
    let mut result = Vec::new();
    let mut skip = false;
    for arg in args {
        if skip {
            skip = false;
            continue;
        }
        if flags.contains(&arg.as_str()) {
            skip = true;
            continue;
        }
        if !arg.starts_with('-') {
            result.push(arg.clone());
        }
    }
    result
}

/// Returns the named argument value, if present.
fn value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == name)
        .map(|w| w[1].clone())
        .or_else(|| {
            args.iter()
                .find_map(|x| x.strip_prefix(&format!("{name}=")).map(str::to_string))
        })
}

/// Creates an invalid-input error.
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
