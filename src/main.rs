mod cli_send;
mod cli_tool_call;

use orchid::{
    config::{init, Settings},
    model::{Session, Status},
    store::{default_root, Store},
};
use std::{
    env, io,
    path::PathBuf,
    process::{Command, ExitCode},
    time::{Duration, Instant},
};

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

fn run(args: Vec<String>) -> io::Result<String> {
    let (root, args) = root_arg(args);
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
    ("tool", |a, _, c| orchid::tools::command(c, a)),
    ("auth", |a, _, c| orchid::config::auth(c, a)),
    ("__run", |a, s, c| orchid::provider::command(s, c, a)),
    ("__hook-run", |a, _, c| {
        orchid::hooks::monitor(c, a).map(|_| String::new())
    }),
    ("await", |a, s, c| await_sessions(s, c, a)),
    ("stop", |a, s, c| stop(s, c, a)),
    ("agent", |_, _, c| agent(c)),
    ("session", |a, s, c| session(s, c, a)),
];

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

fn create(store: &Store, settings: &Settings, args: &[String]) -> io::Result<String> {
    let label = value(args, "--label");
    let working_dir = value(args, "--working-dir");
    let agent = value(args, "--agent").unwrap_or_else(|| "default".into());
    let snapshot = settings.snapshot_agent(&agent)?;
    let prompt = settings.resolve_agent(&agent)?.prompt()?;
    let session = Session::new(label, working_dir, Some(snapshot));
    let id = session.metadata.id.clone();
    store.create_with_prompt(&session, &prompt)?;
    Ok(serde_json::json!({"id":id,"status":"idle","agent":agent}).to_string())
}

fn agent(settings: &Settings) -> io::Result<String> {
    serde_json::to_string(&serde_json::json!({"agents": settings.agent_summaries()?}))
        .map_err(io::Error::other)
}

fn session(store: &Store, settings: &Settings, args: &[String]) -> io::Result<String> {
    let id = args
        .first()
        .ok_or_else(|| invalid("session requires an id"))?;
    let agent = value(&args[1..], "--agent").ok_or_else(|| invalid("session requires --agent"))?;
    let resolved = settings.resolve_agent(&agent)?;
    let prompt = resolved.prompt()?;
    store.update_with_prompt(id, &prompt, |session| {
        session.metadata.policy = resolved.policy.clone();
    })?;
    Ok(serde_json::json!({"id":id,"agent":agent,"updated":true}).to_string())
}

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

fn set(store: &Store, args: &[String]) -> io::Result<String> {
    let id = args
        .first()
        .ok_or_else(|| invalid("set requires an id"))?
        .clone();
    let label = value(&args[1..], "--label");
    let dir = value(&args[1..], "--working-dir");
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

fn delete(store: &Store, args: &[String]) -> io::Result<String> {
    let id = args
        .first()
        .ok_or_else(|| invalid("delete requires an id"))?;
    store.archive(id)?;
    Ok(serde_json::json!({"id":id,"archived":true}).to_string())
}

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
                    let _ = orchid::hooks::dispatch_events(settings, &session, before);
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

fn stop(store: &Store, settings: &Settings, args: &[String]) -> io::Result<String> {
    let id = args.first().ok_or_else(|| invalid("stop requires an id"))?;
    let session = store.load(id)?;
    if let Some(pid) = session.metadata.pid {
        terminate_worker(pid);
    }

    let mut session = store.load(id)?;
    for call_id in session.pending_tool_call_ids() {
        orchid::hooks::append(
            store,
            settings,
            &mut session,
            orchid::model::Event::ToolResult {
                event_id: uuid::Uuid::new_v4().to_string(),
                timestamp: chrono::Utc::now(),
                call_id,
                content: serde_json::json!({
                    "status": "terminated",
                    "error": "tool call terminated because the session was cancelled"
                }),
                token_usage: orchid::model::TokenUsage::default(),
            },
        )?;
    }
    orchid::hooks::append(
        store,
        settings,
        &mut session,
        orchid::model::Event::Termination {
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            reason: "cancelled by user".into(),
            token_usage: orchid::model::TokenUsage::default(),
        },
    )?;
    store.update(id, |session| {
        session.metadata.status = Status::Idle;
        session.metadata.pid = None;
        session.metadata.termination_reason = Some("cancelled by user".into());
    })?;
    Ok(serde_json::json!({"id":id,"status":"idle"}).to_string())
}

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

fn process_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

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
fn help() -> String {
    "orchid <create|list|get|set|delete|send|tool-call|agent|session>\n  --config DIR  configuration/session root".into()
}
