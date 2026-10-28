//! `await` and `stop`: observing and ending session workers.
use crate::cli_args::{invalid, positional, value};
use orchid::{
    config::Settings,
    model::{Event, EventKind, Status},
    store::{process_alive, Store},
};
use std::{
    io,
    process::Command,
    time::{Duration, Instant},
};

/// Performs the await sessions operation.
pub fn await_sessions(store: &Store, settings: &Settings, args: &[String]) -> io::Result<String> {
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
                // Fire hooks only for events reconcile itself added; a live
                // worker fires its own.
                let (session, added) = store.reconcile(id)?;
                if let Some(from) = added {
                    let settings = settings.for_session(&session);
                    let _ = orchid::hooks::fire_events(&settings, &session, from);
                }
                Ok(session)
            })
            .collect::<io::Result<_>>()?;
        if sessions
            .iter()
            .all(|s| s.metadata.status != Status::Running)
            || Instant::now() >= deadline
        {
            // Exit 0 when every session is idle, 1 if any failed, was cancelled
            // or terminated, 2 on timeout; the JSON is printed either way.
            let code = sessions.iter().fold(0, |code, s| match s.metadata.status {
                Status::Idle => code,
                Status::Running => 2,
                _ => code.max(1),
            });
            let statuses: Vec<_> = sessions
                .iter()
                .map(|s| {
                    let idle = s.metadata.status == Status::Idle;
                    serde_json::json!({"id": s.metadata.id, "status": s.metadata.status,
                        "message": s.last_message().filter(|_| idle), "reason": s.metadata.termination_reason})
                })
                .collect();
            let output = serde_json::json!({"sessions": statuses}).to_string();
            if code != 0 {
                println!("{output}");
                std::process::exit(code);
            }
            return Ok(output);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Stops a session's worker, answers open tool calls, and records the cancellation.
/// A session without a worker is left unchanged.
pub fn stop(store: &Store, settings: &Settings, args: &[String]) -> io::Result<String> {
    let id = args.first().ok_or_else(|| invalid("stop requires an id"))?;
    let session = store.load(id)?;
    if session.metadata.status != Status::Running && session.metadata.pid.is_none() {
        return Ok(serde_json::json!({"id":id,"status":session.metadata.status}).to_string());
    }
    let killed = session.metadata.pid;
    if let Some(pid) = killed {
        terminate_worker(pid);
    }
    // One locked update, skipped if a new worker claimed the session meanwhile.
    let mut before = None;
    let session = store.update(id, |session| {
        if session.metadata.pid != killed && session.metadata.pid.is_some_and(process_alive) {
            return;
        }
        before = Some(session.events.len());
        session.close_pending("tool call terminated because the session was cancelled");
        let reason = "cancelled by user".into();
        session.append(Event::new(EventKind::Termination { reason }));
        session.metadata.status = Status::Cancelled;
        session.metadata.pid = None;
        session.metadata.termination_reason = Some("cancelled by user".into());
    })?;
    if let Some(from) = before {
        let _ = orchid::hooks::fire_events(&settings.for_session(&session), &session, from);
    }
    Ok(serde_json::json!({"id":id,"status":session.metadata.status}).to_string())
}

/// Signals a worker's process group (its tool commands included) and the
/// worker itself, escalating to SIGKILL after two seconds.
fn terminate_worker(pid: u32) {
    let signal = |name: &str| {
        let targets = [format!("-{pid}"), pid.to_string()];
        let _ = Command::new("kill")
            .args(["-s", name, "--", &targets[0], &targets[1]])
            .stderr(std::process::Stdio::null())
            .status();
    };
    signal("TERM");
    let deadline = Instant::now() + Duration::from_secs(2);
    while process_alive(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(25));
    }
    if process_alive(pid) {
        signal("KILL");
    }
}
