use crate::{
    config::{HookDefinition, HookMode, Settings},
    model::{Event, Session},
    store::Store,
};
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
struct Envelope<'a> {
    version: u8,
    event: serde_json::Value,
    session: &'a Session,
}

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
        event: json!({"name": name, "event_id": event_id(trigger), "event_type": event_type(trigger)}),
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

fn launch_async(
    settings: &Settings,
    session_id: &str,
    event_name: &str,
    hook: &HookDefinition,
    input: &[u8],
    working_dir: Option<&str>,
) -> io::Result<()> {
    let _ = working_dir;
    #[cfg(test)]
    {
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
        return Ok(());
    }
    #[cfg(not(test))]
    {
        let input_path = settings
            .root
            .join("sessions")
            .join(session_id)
            .join(format!(".hook-input-{}.json", uuid::Uuid::new_v4()));
        fs::write(&input_path, input)?;
        let timeout = hook
            .timeout_seconds
            .or(settings.policy.hooks.timeout)
            .unwrap_or(30);
        let child = Command::new(std::env::current_exe()?)
            .arg("--config")
            .arg(&settings.root)
            .arg("__hook-run")
            .arg("--id")
            .arg(session_id)
            .arg("--event")
            .arg(event_name)
            .arg("--script")
            .arg(&hook.script)
            .arg("--input")
            .arg(&input_path)
            .arg("--timeout")
            .arg(timeout.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        log_lifecycle(
            settings,
            session_id,
            "async hook monitor launched",
            "info",
            json!({"event":event_name,"script":hook.script,"pid":child.id()}),
        );
        Ok(())
    }
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
    let event = event;
    *session = store.append_event(&session.metadata.id, event.clone())?;
    if hook_depth(&settings.root, &session.metadata.id) >= 8 {
        return Ok(());
    }
    let mut names = vec!["on-event"];
    if first {
        names.push("on-init");
    }
    match &event {
        Event::ToolCall { .. } => names.push("on-tool-call"),
        Event::ToolResult { .. } => names.push("on-tool-result"),
        Event::Failure { .. } | Event::Termination { .. } => names.push("on-error"),
        _ => {}
    }
    for name in names {
        dispatch(settings, name, &event, session)?;
    }
    Ok(())
}

pub fn dispatch_events(settings: &Settings, session: &Session, from: usize) -> io::Result<()> {
    for event in session.events.iter().skip(from) {
        let mut names = vec!["on-event"];
        match event {
            Event::ToolCall { .. } => names.push("on-tool-call"),
            Event::ToolResult { .. } => names.push("on-tool-result"),
            Event::Failure { .. } | Event::Termination { .. } => names.push("on-error"),
            _ => {}
        }
        for name in names {
            dispatch(settings, name, event, session)?;
        }
    }
    Ok(())
}

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
    let _depth = HookDepth::enter(&settings.root, session_id)?;
    let executable = resolve_executable(settings, &hook.script);
    let mut child = match Command::new(executable)
        .current_dir(resolve_working_dir(settings, working_dir))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
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
    let stdout_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.take(1024 * 1024).read_to_end(&mut bytes);
        bytes
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.take(1024 * 1024).read_to_end(&mut bytes);
        bytes
    });
    child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("hook stdin unavailable"))?
        .write_all(input)?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            let stdout =
                String::from_utf8_lossy(&stdout_reader.join().unwrap_or_default()).into_owned();
            let stderr =
                String::from_utf8_lossy(&stderr_reader.join().unwrap_or_default()).into_owned();
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
            return Err(error);
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
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

fn resolve_executable(settings: &Settings, script: &str) -> std::path::PathBuf {
    let path = std::path::Path::new(script);
    let local = settings.root.join(path);
    if path.is_absolute() || path.components().count() > 1 || is_executable(&local) {
        local.canonicalize().unwrap_or(local)
    } else {
        path.to_path_buf()
    }
}

fn resolve_working_dir(settings: &Settings, working_dir: Option<&str>) -> std::path::PathBuf {
    let Some(working_dir) = working_dir else {
        return settings.root.clone();
    };
    let path = std::path::Path::new(working_dir);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        settings.root.join(path)
    }
}

fn is_executable(path: &std::path::Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn hook_depth(root: &std::path::Path, session_id: &str) -> u32 {
    fs::read_to_string(root.join("sessions").join(session_id).join(".hook-depth"))
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

struct HookDepth {
    path: std::path::PathBuf,
}

impl HookDepth {
    fn enter(root: &std::path::Path, session_id: &str) -> io::Result<Self> {
        let dir = root.join("sessions").join(session_id);
        fs::create_dir_all(&dir)?;
        let path = dir.join(".hook-depth");
        let depth = hook_depth(root, session_id) + 1;
        fs::write(&path, depth.to_string())?;
        Ok(Self { path })
    }
}

impl Drop for HookDepth {
    fn drop(&mut self) {
        let depth = fs::read_to_string(&self.path)
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(1);
        if depth <= 1 {
            let _ = fs::remove_file(&self.path);
        } else {
            let _ = fs::write(&self.path, (depth - 1).to_string());
        }
    }
}

fn format_mode(mode: &HookMode) -> &'static str {
    match mode {
        HookMode::Sync => "sync",
        HookMode::Async => "async",
    }
}

fn log_lifecycle(
    settings: &Settings,
    session_id: &str,
    message: &str,
    level: &str,
    fields: serde_json::Value,
) {
    let Ok(store) = Store::new(&settings.root) else {
        return;
    };
    let _ = store.log_both(
        session_id,
        &crate::model::LogRecord {
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            level: level.into(),
            message: message.into(),
            fields,
        },
        &settings.log_level,
    );
}

fn event_id(event: &Event) -> &str {
    match event {
        Event::Message { event_id, .. }
        | Event::ToolCall { event_id, .. }
        | Event::ToolResult { event_id, .. }
        | Event::Reasoning { event_id, .. }
        | Event::Usage { event_id, .. }
        | Event::Termination { event_id, .. }
        | Event::Failure { event_id, .. } => event_id,
    }
}

fn event_type(event: &Event) -> &'static str {
    match event {
        Event::Message { .. } => "message",
        Event::ToolCall { .. } => "tool_call",
        Event::ToolResult { .. } => "tool_result",
        Event::Reasoning { .. } => "reasoning",
        Event::Usage { .. } => "usage",
        Event::Termination { .. } => "termination",
        Event::Failure { .. } => "failure",
    }
}

pub fn run(settings: &Settings, event: &str, session_id: &str) -> io::Result<()> {
    let store = crate::store::Store::new(&settings.root)?;
    let session = store.load(session_id)?;
    let trigger = session
        .events
        .last()
        .ok_or_else(|| io::Error::other("session has no events"))?;
    dispatch(settings, event, trigger, &session)
}

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

fn value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
#[path = "hooks_tests.rs"]
mod tests;
