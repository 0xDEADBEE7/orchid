use orchid_simplified::{
    config::{init, Settings},
    model::{Session, Status},
    store::{default_root, Store},
};
use std::{
    env, io,
    path::PathBuf,
    process::{Command, ExitCode, Stdio},
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
    dispatch(
        args.first().map(String::as_str),
        &args[1..],
        &store,
        &settings,
    )
}

type Handler = fn(&[String], &Store, &Settings) -> io::Result<String>;
const COMMANDS: &[(&str, Handler)] = &[
    ("help", |_, _, _| Ok(help())),
    ("create", |a, s, _| create(s, a)),
    ("list", |_, s, _| list(s)),
    ("get", |a, s, _| get(s, a)),
    ("set", |a, s, _| set(s, a)),
    ("delete", |a, s, _| delete(s, a)),
    ("send", |a, s, c| send(s, c, a)),
    ("tool", |a, _, c| orchid_simplified::tools::command(c, a)),
    ("config", |a, _, c| orchid_simplified::config::command(c, a)),
    ("auth", |a, _, c| orchid_simplified::config::auth(c, a)),
    ("__run", |a, s, c| {
        orchid_simplified::provider::command(s, c, a)
    }),
    ("await", |a, s, _| await_sessions(s, a)),
    ("stop", |a, s, _| stop(s, a)),
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
    let mut root = default_root().to_path_buf();
    if let Some(i) = args.iter().position(|x| x == "--config") {
        if i + 1 < args.len() {
            root = PathBuf::from(args.remove(i + 1));
            args.remove(i);
        }
    }
    (root, args)
}

fn create(store: &Store, args: &[String]) -> io::Result<String> {
    let label = value(args, "--label");
    let working_dir = value(args, "--working-dir");
    let policy = value(args, "--policy");
    let session = Session::new(label, working_dir, policy);
    let id = session.metadata.id.clone();
    store.create(&session)?;
    Ok(id)
}

fn list(store: &Store) -> io::Result<String> {
    let mut sessions = store.list()?;
    sessions.sort_by_key(|s| s.metadata.created_at);
    serde_json::to_string(&sessions).map_err(io::Error::other)
}

fn get(store: &Store, args: &[String]) -> io::Result<String> {
    let id = args.first().ok_or_else(|| invalid("get requires an id"))?;
    let session = store.load(id)?;
    if args.iter().any(|x| x == "--last-message") {
        return Ok(session.state.last_message.unwrap_or_default());
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
    Ok(id)
}

fn delete(store: &Store, args: &[String]) -> io::Result<String> {
    let id = args
        .first()
        .ok_or_else(|| invalid("delete requires an id"))?;
    store.archive(id)?;
    Ok(id.to_string())
}

fn send(store: &Store, settings: &Settings, args: &[String]) -> io::Result<String> {
    let message = positional(args)
        .first()
        .ok_or_else(|| invalid("send requires a message"))?
        .clone();
    let id = value(args, "--id").ok_or_else(|| invalid("send requires --id"))?;
    orchid_simplified::hooks::run(settings, "run_start", &id)?;
    if args.iter().any(|arg| arg == "--await") {
        return Err(invalid("send does not support --await; use await <ID>"));
    }
    let mut session = store.load(&id)?;
    // Persist the user event before starting the asynchronous worker so the
    // send command is immediately observable in events.jsonl.
    session.append(orchid_simplified::model::Event::Message {
        role: "user".into(),
        content: message.clone(),
    });
    session.state.status = Status::Running;
    let exe = env::current_exe()?;
    let child = Command::new(exe)
        .arg("--config")
        .arg(&settings.root)
        .arg("__run")
        .arg("--id")
        .arg(&id)
        .arg(&message)
        // The worker persists the model response in the session store. It
        // must never inherit the CLI's stdout/stderr and corrupt its JSON
        // protocol with assistant text (or worker diagnostics).
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    session.state.pid = Some(child.id());
    store.save(&session)?;
    Ok(serde_json::json!({
        "id": id,
        "status": Status::Running,
        "pid": child.id(),
    })
    .to_string())
}

fn await_sessions(store: &Store, args: &[String]) -> io::Result<String> {
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
            .map(|id| store.reconcile(id))
            .collect::<io::Result<_>>()?;
        if sessions.iter().all(|s| s.state.status != Status::Running) || Instant::now() >= deadline
        {
            let statuses: Vec<_> = sessions
                .iter()
                .zip(&ids)
                .map(|(s, id)| serde_json::json!({"id": id, "status": s.state.status}))
                .collect();
            return serde_json::to_string(&statuses).map_err(io::Error::other);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn stop(store: &Store, args: &[String]) -> io::Result<String> {
    let id = args.first().ok_or_else(|| invalid("stop requires an id"))?;
    let session = store.load(id)?;
    if let Some(pid) = session.state.pid {
        let _ = Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .status();
    }
    store.update(id, |s| {
        s.state.status = Status::Cancelled;
        s.state.pid = None;
    })?;
    Ok(id.clone())
}

fn positional(args: &[String]) -> Vec<String> {
    let value_flags = [
        "--id",
        "--label",
        "--working-dir",
        "--policy",
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
    "orchid-simplified <create|list|get|set|delete|send|config>\n  --config DIR  configuration/session root".into()
}
