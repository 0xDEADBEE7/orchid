use crate::{config::{HookDefinition, HookMode, Settings}, model::{Event, Session}, store::Store};
use serde::Serialize;
use serde_json::json;
use std::{io::{self, Write}, process::{Command, Stdio}, thread, time::{Duration, Instant}};

#[derive(Debug, Serialize)]
struct Envelope<'a> {
    version: u8,
    event: serde_json::Value,
    session: &'a Session,
}

pub fn dispatch(settings: &Settings, name: &str, trigger: &Event, session: &Session) -> io::Result<()> {
    let Some(hooks) = settings.policy.hooks.events.get(name) else { return Ok(()) };
    let input = serde_json::to_vec(&Envelope {
        version: 1,
        event: json!({"name": name, "event_id": event_id(trigger), "event_type": event_type(trigger)}),
        session,
    }).map_err(io::Error::other)?;
    for hook in hooks {
        match hook.mode {
            HookMode::Sync => run_one(settings, hook, &input)?,
            HookMode::Async => {
                let settings = settings.clone();
                let hook = hook.clone();
                let input = input.clone();
                thread::spawn(move || { let _ = run_one(&settings, &hook, &input); });
            }
        }
    }
    Ok(())
}

/// Append an event durably, then dispatch the hooks that match it. The store
/// write intentionally happens before any hook process is started.
pub fn append(store: &Store, settings: &Settings, session: &mut Session, event: Event) -> io::Result<()> {
    let first = session.events.is_empty();
    session.append(event.clone());
    store.save(session)?;
    let mut names = vec!["on-event"];
    if first { names.push("on-init"); }
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
        for name in names { dispatch(settings, name, event, session)?; }
    }
    Ok(())
}

fn run_one(settings: &Settings, hook: &HookDefinition, input: &[u8]) -> io::Result<()> {
    let timeout = Duration::from_secs(hook.timeout_seconds.or(settings.policy.hooks.timeout).unwrap_or(30));
    let mut child = Command::new(settings.root.join(&hook.script))
        .current_dir(&settings.root)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    child.stdin.take().ok_or_else(|| io::Error::other("hook stdin unavailable"))?.write_all(input)?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            if status.success() { return Ok(()) }
            return Err(io::Error::other(format!("hook exited with status {status}")));
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(io::ErrorKind::TimedOut, "hook timed out"));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn event_id(event: &Event) -> &str {
    match event {
        Event::Message { event_id, .. } | Event::ToolCall { event_id, .. } |
        Event::ToolResult { event_id, .. } | Event::Reasoning { event_id, .. } |
        Event::Usage { event_id, .. } | Event::Termination { event_id, .. } |
        Event::Failure { event_id, .. } => event_id,
    }
}

fn event_type(event: &Event) -> &'static str {
    match event {
        Event::Message { .. } => "message", Event::ToolCall { .. } => "tool_call",
        Event::ToolResult { .. } => "tool_result", Event::Reasoning { .. } => "reasoning",
        Event::Usage { .. } => "usage", Event::Termination { .. } => "termination",
        Event::Failure { .. } => "failure",
    }
}

pub fn run(settings: &Settings, event: &str, session_id: &str) -> io::Result<()> {
    let store = crate::store::Store::new(&settings.root)?;
    let session = store.load(session_id)?;
    let trigger = session.events.last().ok_or_else(|| io::Error::other("session has no events"))?;
    dispatch(settings, event, trigger, &session)
}
