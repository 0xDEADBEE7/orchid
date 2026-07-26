use crate::{
    config::{HookDefinition, HookMode, Settings},
    model::{Event, Session},
    store::Store,
};
use serde::Serialize;
use serde_json::json;
use std::{
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
            HookMode::Sync => run_one(settings, &session.metadata.id, name, hook, &input)?,
            HookMode::Async => {
                let settings = settings.clone();
                let hook = hook.clone();
                let input = input.clone();
                let session_id = session.metadata.id.clone();
                let event_name = name.to_owned();
                thread::spawn(move || {
                    let _ = run_one(&settings, &session_id, &event_name, &hook, &input);
                });
            }
        }
    }
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
    let event = event;
    *session = store.append_event(&session.metadata.id, event.clone())?;
    if hook_depth() >= 8 {
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
    let mut child = Command::new(settings.root.join(&hook.script))
        .current_dir(&settings.root)
        .env("ORCHID_INTERNAL_HOOK_DEPTH", (hook_depth() + 1).to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
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
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            if status.success() {
                log_lifecycle(
                    settings,
                    session_id,
                    "hook completed",
                    "info",
                    json!({"event":event_name,"script":hook.script,"status":status.code()}),
                );
                return Ok(());
            }
            let error = io::Error::other(format!("hook exited with status {status}"));
            log_lifecycle(
                settings,
                session_id,
                "hook failed",
                "error",
                json!({"event":event_name,"script":hook.script,"error":error.to_string()}),
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

fn hook_depth() -> u32 {
    std::env::var("ORCHID_INTERNAL_HOOK_DEPTH")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
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

#[cfg(test)]
#[path = "hooks_tests.rs"]
mod tests;
