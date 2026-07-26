use orchid::{
    config::Settings,
    model::{Session, Status},
    store::Store,
};
use std::{
    env,
    fs::OpenOptions,
    io,
    process::{Command, Stdio},
};

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

fn run_send(
    store: &Store,
    settings: &Settings,
    id: &str,
    message: &str,
    no_run: bool,
) -> io::Result<String> {
    let mut session = store.load(id)?;
    if session.metadata.status == Status::Running {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "session is already running",
        ));
    }
    orchid::hooks::append(
        store,
        settings,
        &mut session,
        Session::message("user", message.to_owned()),
    )?;
    if no_run {
        return Ok(
        serde_json::json!({"id":id,"status":session.metadata.status,"no_run":true}).to_string(),
        );
    }
    // A synchronous hook may append another event through `send --no-run`.
    // Refresh before changing the running state so the outer process never
    // attempts to save a stale, shorter event stream.
    session = store.load(id)?;
    session.metadata.status = Status::Running;
    log(store, settings, id, "info", "send accepted");
    let child = match spawn_worker(settings, id, message) {
        Ok(child) => child,
        Err(error) => {
            log(
                store,
                settings,
                id,
                "error",
                &format!("worker spawn failed: {error}"),
            );
            return Err(error);
        }
    };
    session.metadata.pid = Some(child.id());
    store.save(&session)?;
    log(
        store,
        settings,
        id,
        "info",
        &format!("worker spawned: {}", child.id()),
    );
    Ok(serde_json::json!({"id":id,"status":Status::Running,"pid":child.id()}).to_string())
}

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

fn log(store: &Store, settings: &Settings, id: &str, level: &str, message: &str) {
    let _ = store.log_both(
        id,
        &orchid::model::LogRecord {
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            level: level.into(),
            message: message.into(),
            fields: serde_json::json!({}),
        },
        &settings.log_level,
    );
}

fn spawn_worker(settings: &Settings, id: &str, message: &str) -> io::Result<std::process::Child> {
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
        .arg(message)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr)
        .spawn()
}

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

fn value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == name)
        .map(|w| w[1].clone())
        .or_else(|| {
            args.iter()
                .find_map(|x| x.strip_prefix(&format!("{name}=")).map(str::to_string))
        })
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
