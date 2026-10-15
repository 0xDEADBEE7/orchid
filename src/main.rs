//! Provides the main functionality.
mod cli_send;
mod cli_tool_call;

use orchid::{
    config::{init, Settings},
    model::{Session, Status},
    store::{default_root, process_alive, Store},
};
use std::{
    env, io,
    path::PathBuf,
    process::{Command, ExitCode},
    time::{Duration, Instant},
};

/// Performs the main operation.
fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "{}",
                serde_json::json!({"error": "command_failed", "message": error.to_string()})
            );
            ExitCode::from(1)
        }
    }
}

/// Runs the requested operation.
fn run(args: Vec<String>) -> io::Result<String> {
    let (root, args) = root_arg(args);
    if args.len() == 1 && matches!(args[0].as_str(), "--version" | "-V") {
        return Ok(format!(
            "orchid {} (codex compatibility {})",
            env!("CARGO_PKG_VERSION"),
            orchid::client::openai_codex::compatibility::VERSION
        ));
    }
    init(&root)?;
    let settings = Settings::load(&root)?;
    let store = Store::new(&root)?;
    let (command, command_args) = args
        .split_first()
        .map_or((None, &[][..]), |(command, args)| {
            let command = match command.as_str() {
                "-h" | "--help" => "help",
                other => other,
            };
            (Some(command), args)
        });
    dispatch(command, command_args, &store, &settings)
}

type Handler = fn(&[String], &Store, &Settings) -> io::Result<String>;
const COMMANDS: &[(&str, Handler)] = &[
    ("help", |_, _, _| {
        Ok(serde_json::json!({"ok":true,"help":help()}).to_string())
    }),
    ("create", |a, s, c| create(s, c, a)),
    ("list", |_, s, _| list(s)),
    ("get", |a, s, _| get(s, a)),
    ("set", |a, s, _| set(s, a)),
    ("delete", |a, s, _| delete(s, a)),
    ("send", |a, s, c| cli_send::send(s, c, a)),
    ("tool-call", |a, s, c| cli_tool_call::tool_call(s, c, a)),
    ("auth", |a, _, c| orchid::config::auth(c, a)),
    ("__run", |a, s, c| orchid::provider::command(s, c, a)),
    ("await", |a, s, c| await_sessions(s, c, a)),
    ("stop", |a, s, c| stop(s, c, a)),
    ("agent", |_, _, c| agent(c)),
    ("session", |a, s, c| session(s, c, a)),
];

/// Dispatches the requested hook or command.
fn dispatch(
    command: Option<&str>,
    args: &[String],
    store: &Store,
    settings: &Settings,
) -> io::Result<String> {
    let name = command.unwrap_or("help");
    let name = if name == "kill" { "stop" } else { name };
    COMMANDS
        .iter()
        .find(|(known, _)| *known == name)
        .map_or_else(
            || {
                Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown command: {name}"),
                ))
            },
            |(_, handler)| handler(args, store, settings),
        )
}

/// Performs the root arg operation.
fn root_arg(mut args: Vec<String>) -> (PathBuf, Vec<String>) {
    let mut root = default_root();
    if let Some(i) = args.iter().position(|x| x == "--config") {
        if i + 1 < args.len() {
            root = PathBuf::from(args.remove(i + 1));
            args.remove(i);
        }
    }
    (root, args)
}

/// Performs the create operation.
fn create(store: &Store, settings: &Settings, args: &[String]) -> io::Result<String> {
    let label = value(args, "--label");
    let working_dir = value(args, "--working-dir")
        .map(canonical_dir)
        .transpose()?;
    let name = value(args, "--agent").unwrap_or_else(|| "default".into());
    let mut agent = settings.agent_named(&name)?;
    if let Some(connection) = value(args, "--connection") {
        agent.connection = connection;
    }
    let prompt = settings.prompt_text(&agent.prompt)?;
    let mut session = Session::new(label, working_dir, Some(agent));
    session.metadata.agent_name = Some(name.clone());
    let id = session.metadata.id.clone();
    store.create_with_prompt(&session, &prompt)?;
    Ok(serde_json::json!({"id":id,"status":"idle","agent":name}).to_string())
}

/// Performs the agent operation.
fn agent(settings: &Settings) -> io::Result<String> {
    serde_json::to_string(&serde_json::json!({"agents": settings.agent_summaries()?}))
        .map_err(io::Error::other)
}

/// Performs the session operation.
fn session(store: &Store, settings: &Settings, args: &[String]) -> io::Result<String> {
    let id = args
        .first()
        .ok_or_else(|| invalid("session requires an id"))?;
    let name = value(&args[1..], "--agent").ok_or_else(|| invalid("session requires --agent"))?;
    let agent = settings.agent_named(&name)?;
    let prompt = settings.prompt_text(&agent.prompt)?;
    store.update_with_prompt(id, &prompt, |session| {
        session.metadata.agent = agent;
        session.metadata.agent_name = Some(name.clone());
    })?;
    Ok(serde_json::json!({"id":id,"agent":name,"updated":true}).to_string())
}

/// Lists the available entries.
fn list(store: &Store) -> io::Result<String> {
    let mut sessions = store.list()?;
    sessions.sort_by_key(|s| s.metadata.created_at);
    let sessions: Vec<_> = sessions
        .into_iter()
        .map(|session| {
            serde_json::json!({
                "id": session.metadata.id,
                "label": session.metadata.label,
            })
        })
        .collect();
    serde_json::to_string(&serde_json::json!({"sessions":sessions})).map_err(io::Error::other)
}

/// Performs the get operation.
fn get(store: &Store, args: &[String]) -> io::Result<String> {
    let id = args.first().ok_or_else(|| invalid("get requires an id"))?;
    let session = store.load(id)?;
    if args.iter().any(|x| x == "--last-message") {
        return Ok(
            serde_json::json!({"id":id,"last_message":session.events.iter().rev().find_map(|event| match event { orchid::model::Event::Message { role, content, .. } if role == "assistant" => Some(content), _ => None })}).to_string(),
        );
    }
    serde_json::to_string(&session).map_err(io::Error::other)
}

/// Performs the set operation.
fn set(store: &Store, args: &[String]) -> io::Result<String> {
    let id = args
        .first()
        .ok_or_else(|| invalid("set requires an id"))?
        .clone();
    if let Some(flag) = args[1..]
        .iter()
        .filter(|arg| arg.starts_with("--"))
        .find(|arg| !matches!(arg.split('=').next(), Some("--label" | "--working-dir")))
    {
        return Err(invalid(&format!("set: unknown flag {flag}")));
    }
    let label = value(&args[1..], "--label");
    let dir = value(&args[1..], "--working-dir")
        .map(canonical_dir)
        .transpose()?;
    store.update(&id, |s| {
        if label.is_some() {
            s.metadata.label = label;
        }
        if dir.is_some() {
            s.metadata.working_dir = dir;
        }
    })?;
    Ok(serde_json::json!({"id":id,"updated":true}).to_string())
}

/// Performs the delete operation.
fn delete(store: &Store, args: &[String]) -> io::Result<String> {
    let id = args
        .first()
        .ok_or_else(|| invalid("delete requires an id"))?;
    store.archive(id)?;
    Ok(serde_json::json!({"id":id,"archived":true}).to_string())
}

/// Performs the await sessions operation.
fn await_sessions(store: &Store, settings: &Settings, args: &[String]) -> io::Result<String> {
    if args.is_empty() {
        return Err(invalid("await requires at least one id"));
    }
    let ids = positional(args);
    let timeout = value(args, "--timeout")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(60.0);
    let deadline = Instant::now() + Duration::from_secs_f64(timeout.max(0.0));
    loop {
        let sessions: Vec<_> = ids
            .iter()
            .map(|id| {
                let before = store.load(id)?.events.len();
                let session = store.reconcile(id)?;
                if session.events.len() > before {
                    let settings = settings.for_session(&session);
                    let _ = orchid::hooks::dispatch_events(&settings, &session, before);
                }
                Ok(session)
            })
            .collect::<io::Result<_>>()?;
        if sessions
            .iter()
            .all(|s| s.metadata.status != Status::Running)
            || Instant::now() >= deadline
        {
            let statuses: Vec<_> = sessions
                .iter()
                .zip(&ids)
                .map(|(s, id)| serde_json::json!({"id": id, "status": s.metadata.status}))
                .collect();
            return serde_json::to_string(&serde_json::json!({"sessions":statuses}))
                .map_err(io::Error::other);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Stops a session's worker, answers open tool calls, and records the cancellation.
/// A session without a worker is left unchanged.
fn stop(store: &Store, settings: &Settings, args: &[String]) -> io::Result<String> {
    let id = args.first().ok_or_else(|| invalid("stop requires an id"))?;
    let session = store.load(id)?;
    if session.metadata.status != Status::Running && session.metadata.pid.is_none() {
        return Ok(serde_json::json!({"id":id,"status":session.metadata.status}).to_string());
    }
    if let Some(pid) = session.metadata.pid {
        terminate_worker(pid);
    }
    let mut session = store.load(id)?;
    let before = session.events.len();
    session.close_pending("tool call terminated because the session was cancelled");
    session.append(orchid::model::Event::Termination {
        event_id: uuid::Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now(),
        reason: "cancelled by user".into(),
        connection: None,
        token_usage: orchid::model::TokenUsage::default(),
    });
    session.metadata.status = Status::Idle;
    session.metadata.pid = None;
    session.metadata.termination_reason = Some("cancelled by user".into());
    store.save(&session)?;
    let _ = orchid::hooks::dispatch_events(&settings.for_session(&session), &session, before);
    Ok(serde_json::json!({"id":id,"status":"idle"}).to_string())
}

/// Performs the terminate worker operation.
fn terminate_worker(pid: u32) {
    let _ = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status();
    let deadline = Instant::now() + Duration::from_secs(2);
    while process_alive(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(25));
    }
    if process_alive(pid) {
        let _ = Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .status();
    }
}

/// Performs the positional operation.
fn positional(args: &[String]) -> Vec<String> {
    let value_flags = [
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
        if value_flags.contains(&arg.as_str()) {
            skip = true;
            continue;
        }
        if arg.starts_with('-') {
            continue;
        }
        result.push(arg.clone());
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
/// Resolves a working directory to an existing absolute path, so tools and hooks
/// agree on it regardless of where a command was run.
fn canonical_dir(dir: String) -> io::Result<String> {
    std::fs::canonicalize(&dir)
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|error| io::Error::new(error.kind(), format!("working directory {dir}: {error}")))
}

/// Creates an invalid-input error.
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
/// Returns command-line help text.
fn help() -> String {
    "orchid <create|list|get|set|delete|send|tool-call|agent|session>\n  --config DIR  configuration/session root".into()
}
