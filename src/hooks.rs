//! Provides the hooks functionality.
mod hook_depth;
#[path = "hook_events.rs"]
mod hook_events;
mod hook_logging;
mod hook_paths;

use crate::{
    config::{HookDefinition, HookMode, Settings},
    model::{Event, Session},
    store::Store,
};
use crate::{
    hook_state::HookState,
    hooks::hook_depth::{depth as hook_depth, HookDepth},
};
use hook_logging::{format_mode, log_lifecycle};
use hook_paths::{executable, working_dir as resolve_working_dir};
use serde::Serialize;
use serde_json::json;
use std::{
    fs,
    io::{self, Read, Write},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Serialize)]
/// Performs the Envelope operation.
struct Envelope<'a> {
    version: u8,
    event: serde_json::Value,
    session: &'a Session,
}
/// Dispatches the requested hook or command.
pub fn dispatch(
    settings: &Settings,
    name: &str,
    trigger: &Event,
    session: &Session,
) -> io::Result<()> {
    let Some(hooks) = settings.policy.hooks.events.get(name) else {
        return Ok(());
    };
    let input = serde_json::to_vec(&Envelope {
        version: 1,
        event: json!({"name": name, "event_id": hook_events::id(trigger), "event_type": hook_events::kind(trigger)}),
        session,
    }).map_err(io::Error::other)?;
    for hook in hooks {
        match hook.mode {
            HookMode::Sync => run_one(
                settings,
                &session.metadata.id,
                name,
                hook,
                &input,
                session.metadata.working_dir.as_deref(),
            )?,
            HookMode::Async => launch_async(
                settings,
                &session.metadata.id,
                name,
                hook,
                &input,
                session.metadata.working_dir.as_deref(),
            )?,
        }
    }
    Ok(())
}
/// Performs the launch async operation.
fn launch_async(
    settings: &Settings,
    session_id: &str,
    event_name: &str,
    hook: &HookDefinition,
    input: &[u8],
    working_dir: Option<&str>,
) -> io::Result<()> {
    let settings = settings.clone();
    let session_id = session_id.to_owned();
    let event_name = event_name.to_owned();
    let hook = hook.clone();
    let input = input.to_vec();
    let working_dir = working_dir.map(str::to_owned);
    thread::spawn(move || {
        let _ = run_one(
            &settings,
            &session_id,
            &event_name,
            &hook,
            &input,
            working_dir.as_deref(),
        );
    });
    Ok(())
}
/// Append an event durably, then dispatch the hooks that match it. The store
/// write intentionally happens before any hook process is started.
pub fn append(
    store: &Store,
    settings: &Settings,
    session: &mut Session,
    event: Event,
) -> io::Result<()> {
    let first = session.events.is_empty();
    *session = store.append_event(&session.metadata.id, event.clone())?;
    if hook_depth(&settings.root, &session.metadata.id) >= 8 {
        return Ok(());
    }
    if first {
        dispatch(settings, "on-init", &event, session)?;
    }
    for name in hook_names(&event) {
        dispatch(settings, name, &event, session)?;
    }
    Ok(())
}

/// Performs the hook names operation.
fn hook_names(event: &Event) -> impl Iterator<Item = &'static str> {
    std::iter::once("on-event").chain(match event {
        Event::ToolCall { .. } => Some("on-tool-call"),
        Event::ToolResult { .. } => Some("on-tool-result"),
        Event::Failure { .. } | Event::Termination { .. } => Some("on-error"),
        _ => None,
    })
}

/// Performs the dispatch events operation.
pub fn dispatch_events(settings: &Settings, session: &Session, from: usize) -> io::Result<()> {
    for event in session.events.iter().skip(from) {
        for name in hook_names(event) {
            dispatch(settings, name, event, session)?;
        }
    }
    Ok(())
}
/// Performs the run one operation.
fn run_one(
    settings: &Settings,
    session_id: &str,
    event_name: &str,
    hook: &HookDefinition,
    input: &[u8],
    working_dir: Option<&str>,
) -> io::Result<()> {
    let timeout = Duration::from_secs(
        hook.timeout_seconds
            .or(settings.policy.hooks.timeout)
            .unwrap_or(30),
    );
    log_lifecycle(
        settings,
        session_id,
        "hook started",
        "info",
        json!({"event":event_name,"script":hook.script,"mode":format_mode(&hook.mode)}),
    );
    let mut running = spawn_hook(settings, session_id, event_name, hook, working_dir)?;
    running
        .child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("hook stdin unavailable"))?
        .write_all(input)?;
    wait_for_hook(
        settings,
        session_id,
        event_name,
        hook,
        &mut running,
        timeout,
    )
}

/// Performs the RunningHook operation.
struct RunningHook {
    child: std::process::Child,
    stdout: Option<thread::JoinHandle<Vec<u8>>>,
    stderr: Option<thread::JoinHandle<Vec<u8>>>,
    _depth: HookDepth,
    _hook_state: Option<HookState>,
}

/// Performs the spawn hook operation.
fn spawn_hook(
    settings: &Settings,
    session_id: &str,
    event_name: &str,
    hook: &HookDefinition,
    working_dir: Option<&str>,
) -> io::Result<RunningHook> {
    let depth = HookDepth::enter(&settings.root, session_id)?;
    let hook_state = enter_hook_state(settings, session_id, hook)?;
    let token_path = settings
        .root
        .join("sessions")
        .join(session_id)
        .join(".hook-token");
    let mut child = match start_process(settings, session_id, hook, working_dir, &token_path) {
        Ok(child) => child,
        Err(error) => {
            log_lifecycle(
                settings,
                session_id,
                "hook failed to start",
                "error",
                json!({"event":event_name,"script":hook.script,"error":error.to_string()}),
            );
            return Err(error);
        }
    };
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("hook stdout unavailable"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("hook stderr unavailable"))?;
    Ok(RunningHook {
        child,
        stdout: Some(capture_output(stdout)),
        stderr: Some(capture_output(stderr)),
        _depth: depth,
        _hook_state: hook_state,
    })
}

/// Performs the enter hook state operation.
fn enter_hook_state(
    settings: &Settings,
    session_id: &str,
    hook: &HookDefinition,
) -> io::Result<Option<HookState>> {
    let active = matches!(hook.mode, HookMode::Sync)
        && settings
            .root
            .join("sessions")
            .join(session_id)
            .join("metadata.json")
            .exists();
    active
        .then(|| HookState::enter(settings, session_id))
        .transpose()
}

/// Performs the start process operation.
fn start_process(
    settings: &Settings,
    session_id: &str,
    hook: &HookDefinition,
    working_dir: Option<&str>,
    token_path: &std::path::Path,
) -> io::Result<std::process::Child> {
    Command::new(executable(settings, &hook.script))
        .current_dir(resolve_working_dir(settings, working_dir))
        .env("ORCHID_SESSION_ID", session_id)
        .env(
            "ORCHID_HOOK_TOKEN",
            fs::read_to_string(token_path).unwrap_or_default().trim(),
        )
        .env("ORCHID_HOOK_TOKEN_FILE", token_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

/// Performs the capture output operation.
fn capture_output<R: Read + Send + 'static>(reader: R) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = reader.take(1024 * 1024).read_to_end(&mut bytes);
        bytes
    })
}

/// Performs the wait for hook operation.
fn wait_for_hook(
    settings: &Settings,
    session_id: &str,
    event_name: &str,
    hook: &HookDefinition,
    running: &mut RunningHook,
    timeout: Duration,
) -> io::Result<()> {
    let started = Instant::now();
    loop {
        if let Some(status) = running.child.try_wait()? {
            return finish_hook(settings, session_id, event_name, hook, running, status);
        }
        if started.elapsed() >= timeout {
            let _ = running.child.kill();
            let _ = running.child.wait();
            let _ = running.stdout.take().and_then(|reader| reader.join().ok());
            let _ = running.stderr.take().and_then(|reader| reader.join().ok());
            log_lifecycle(
                settings,
                session_id,
                "hook timed out",
                "error",
                json!({"event":event_name,"script":hook.script}),
            );
            return Err(io::Error::new(io::ErrorKind::TimedOut, "hook timed out"));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

/// Performs the finish hook operation.
fn finish_hook(
    settings: &Settings,
    session_id: &str,
    event_name: &str,
    hook: &HookDefinition,
    running: &mut RunningHook,
    status: std::process::ExitStatus,
) -> io::Result<()> {
    let stdout = String::from_utf8_lossy(
        &running
            .stdout
            .take()
            .and_then(|reader| reader.join().ok())
            .unwrap_or_default(),
    )
    .into_owned();
    let stderr = String::from_utf8_lossy(
        &running
            .stderr
            .take()
            .and_then(|reader| reader.join().ok())
            .unwrap_or_default(),
    )
    .into_owned();
    if status.success() {
        log_lifecycle(
            settings,
            session_id,
            "hook completed",
            "info",
            json!({"event":event_name,"script":hook.script,"status":status.code(),"stdout":stdout,"stderr":stderr}),
        );
        return Ok(());
    }
    let error = io::Error::other(format!("hook exited with status {status}"));
    log_lifecycle(
        settings,
        session_id,
        "hook failed",
        "error",
        json!({"event":event_name,"script":hook.script,"error":error.to_string(),"stdout":stdout,"stderr":stderr}),
    );
    Err(error)
}

/// Performs the resolve executable operation.
pub fn resolve_executable(settings: &Settings, script: &str) -> std::path::PathBuf {
    executable(settings, script)
}
/// Runs the requested operation.
pub fn run(settings: &Settings, event: &str, session_id: &str) -> io::Result<()> {
    let store = crate::store::Store::new(&settings.root)?;
    let session = store.load(session_id)?;
    let trigger = session
        .events
        .last()
        .ok_or_else(|| io::Error::other("session has no events"))?;
    dispatch(settings, event, trigger, &session)
}

/// Performs the monitor operation.
pub fn monitor(settings: &Settings, args: &[String]) -> io::Result<String> {
    let id = value(args, "--id").ok_or_else(|| invalid("__hook-run requires --id"))?;
    let event = value(args, "--event").ok_or_else(|| invalid("__hook-run requires --event"))?;
    let script = value(args, "--script").ok_or_else(|| invalid("__hook-run requires --script"))?;
    let input_path =
        value(args, "--input").ok_or_else(|| invalid("__hook-run requires --input"))?;
    let timeout = value(args, "--timeout").and_then(|x| x.parse().ok());
    let input = fs::read(&input_path);
    let _ = fs::remove_file(&input_path);
    let input = input?;
    let session = Store::new(&settings.root)?.load(&id)?;
    let hook = HookDefinition {
        script,
        mode: HookMode::Sync,
        timeout_seconds: timeout,
    };
    run_one(
        settings,
        &id,
        &event,
        &hook,
        &input,
        session.metadata.working_dir.as_deref(),
    )
    .map(|_| String::new())
}

/// Returns the named argument value, if present.
fn value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

/// Creates an invalid-input error.
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
