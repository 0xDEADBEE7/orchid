//! Provides the main functionality.
mod cli_args;
mod cli_control;
mod cli_send;

use cli_args::{invalid, value};
use orchid::{
    config::{init, Settings},
    model::Session,
    store::{default_root, Store},
};
use std::{env, io, path::PathBuf, process::ExitCode};

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
    // Absolute, so hooks and tools (which run elsewhere) reach the same store.
    let root = std::path::absolute(root)?;
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
    ("auth", |a, _, c| orchid::config::auth(c, a)),
    ("__run", |a, s, c| orchid::provider::command(s, c, a)),
    ("await", |a, s, c| cli_control::await_sessions(s, c, a)),
    ("stop", |a, s, c| cli_control::stop(s, c, a)),
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
    // Sessions created from another session's tools or hooks record it as
    // their parent, which the notify-parent hook reports back to.
    session.metadata.parent =
        value(args, "--parent").or_else(|| env::var("ORCHID_SESSION_ID").ok());
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
        return Ok(serde_json::json!({"id":id,"last_message":session.last_message()}).to_string());
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

/// Resolves a working directory to an existing absolute path, so tools and hooks
/// agree on it regardless of where a command was run.
fn canonical_dir(dir: String) -> io::Result<String> {
    std::fs::canonicalize(&dir)
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|error| io::Error::new(error.kind(), format!("working directory {dir}: {error}")))
}

/// Returns command-line help text.
fn help() -> String {
    "orchid <create|list|get|set|delete|send|agent|session|await|stop>\n  --config DIR  configuration/session root".into()
}
