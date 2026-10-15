//! Hooks: scripts run on session events. Events are persisted before their hooks
//! run. A hook that exits 2 blocks the action that triggered it, with its stderr
//! as the reason; any other failure or a timeout is logged and ignored.
use crate::{
    config::{HookDefinition, Settings},
    model::{Event, LogRecord, Session},
    store::Store,
};
use serde_json::json;
use std::{
    env, fs,
    io::{self, Read},
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

/// Nesting limit for hooks that trigger further hooks through the CLI.
const MAX_DEPTH: u32 = 8;
/// Largest stderr kept from a hook, in bytes.
const MAX_STDERR: u64 = 64 * 1024;

/// Runs the event's hooks (global, then the agent's) in order. Returns the
/// reason if one blocked; the remaining hooks then do not run.
pub fn fire(settings: &Settings, name: &str, trigger: &Event, session: &Session) -> Option<String> {
    let depth = depth();
    let hooks: Vec<_> = settings.hooks(name).collect();
    if hooks.is_empty() || depth >= MAX_DEPTH {
        return None;
    }
    let mut event = serde_json::to_value(trigger).unwrap_or_default();
    event["name"] = name.into();
    event["event_type"] = event["type"].clone();
    let input = json!({"version": 1, "event": event, "session": session}).to_string();
    hooks.into_iter().find_map(|hook| {
        outcome(
            settings,
            name,
            hook,
            session,
            run(settings, name, hook, &input, session, depth),
        )
    })
}

/// Fires `on-event` for every event from index `from`. Returns the first block.
pub fn fire_events(settings: &Settings, session: &Session, from: usize) -> Option<String> {
    let events = session.events.get(from..).unwrap_or_default();
    events
        .iter()
        .find_map(|event| fire(settings, "on-event", event, session))
}

/// Logs a hook's result; returns its reason when it blocked (exit 2).
fn outcome(
    settings: &Settings,
    name: &str,
    hook: &HookDefinition,
    session: &Session,
    result: io::Result<(ExitStatus, String)>,
) -> Option<String> {
    let fields =
        |extra: serde_json::Value| json!({"event": name, "script": hook.script, "detail": extra});
    let (level, message, blocked) = match &result {
        Ok((status, stderr)) if status.code() == Some(2) => {
            let reason = Some(stderr.trim())
                .filter(|r| !r.is_empty())
                .unwrap_or("no reason given");
            ("warning", "hook blocked", Some(reason.to_owned()))
        }
        Ok((status, _)) if status.success() => ("info", "hook completed", None),
        Ok(_) => ("error", "hook failed", None),
        Err(_) => ("error", "hook failed", None),
    };
    let detail = match result {
        Ok((status, stderr)) => json!({"status": status.code(), "stderr": stderr}),
        Err(error) => json!({"error": error.to_string()}),
    };
    let record = LogRecord::new(level, message, fields(detail));
    let _ = Store::new(&settings.root)
        .and_then(|store| store.log_both(&session.metadata.id, &record, &settings.log_level));
    blocked
}

/// Runs one hook to completion or timeout, returning its status and stderr.
fn run(
    settings: &Settings,
    name: &str,
    hook: &HookDefinition,
    input: &str,
    session: &Session,
    depth: u32,
) -> io::Result<(ExitStatus, String)> {
    // stdin is an unlinked file, so a hook that never reads it can't block or fail.
    let path = env::temp_dir().join(format!("orchid-hook-{}", uuid::Uuid::new_v4()));
    fs::write(&path, input)?;
    let stdin = fs::File::open(&path);
    let _ = fs::remove_file(&path);
    let working_dir = session.metadata.working_dir.as_deref();
    let mut child = Command::new(executable(settings, &hook.script))
        .current_dir(working_dir.map_or_else(|| settings.root.clone(), PathBuf::from))
        .env("ORCHID_CONFIG", &settings.root)
        .env("ORCHID_SESSION_ID", &session.metadata.id)
        .env("ORCHID_HOOK_EVENT", name)
        .env("ORCHID_HOOK_DEPTH", (depth + 1).to_string())
        .stdin(stdin?)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()?;
    let stderr = child.stderr.take();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut text = String::new();
        if let Some(mut stderr) = stderr {
            let _ = (&mut stderr).take(MAX_STDERR).read_to_string(&mut text);
            let _ = io::copy(&mut stderr, &mut io::sink());
        }
        let _ = sender.send(text);
    });
    let deadline = Instant::now() + Duration::from_secs(hook.timeout.unwrap_or(30));
    loop {
        if let Some(status) = child.try_wait()? {
            // A backgrounded grandchild may hold stderr open; don't wait for it.
            let stderr = receiver
                .recv_timeout(Duration::from_secs(1))
                .unwrap_or_default();
            return Ok((status, stderr));
        }
        if Instant::now() >= deadline {
            let group = format!("-{}", child.id());
            let _ = Command::new("kill").args(["-KILL", &group]).status();
            let _ = child.wait();
            return Err(io::Error::new(io::ErrorKind::TimedOut, "hook timed out"));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

/// Current hook nesting depth, inherited through the environment.
fn depth() -> u32 {
    env::var("ORCHID_HOOK_DEPTH")
        .ok()
        .and_then(|depth| depth.parse().ok())
        .unwrap_or(0)
}

/// Resolves a hook script: paths are relative to the config root; a bare name is
/// a file in the root if executable there, otherwise looked up on PATH.
pub fn executable(settings: &Settings, script: &str) -> PathBuf {
    let path = Path::new(script);
    let local = settings.root.join(path);
    if path.components().count() > 1 || path.is_absolute() || is_executable(&local) {
        local.canonicalize().unwrap_or(local)
    } else {
        path.to_path_buf()
    }
}

/// Whether a path is an executable file.
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}
