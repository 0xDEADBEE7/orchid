use crate::session::{SessionStore, SessionUpdate};
use crate::types::Status;
use chrono::Utc;
use std::path::Path;
use std::process;

pub fn on_run_start(session_id: &str, config_dir: &Path) -> Result<(), String> {
    let store = SessionStore::with_config_dir(config_dir)?;

    let updates = SessionUpdate {
        status: Some(Status::Running),
        pid: Some(Some(process::id())),
        run_started_at: Some(Some(Utc::now())),
        ..Default::default()
    };

    store.update(session_id, updates)?;
    Ok(())
}

pub fn on_run_end(session_id: &str, config_dir: &Path) -> Result<(), String> {
    let store = SessionStore::with_config_dir(config_dir)?;

    let updates = SessionUpdate {
        status: Some(Status::Idle),
        pid: Some(None),
        run_started_at: Some(None),
        last_run_at: Some(Some(Utc::now())),
        ..Default::default()
    };

    store.update(session_id, updates)?;
    Ok(())
}

pub fn reconcile_crashed(session_id: &str, config_dir: &Path) -> Result<(), String> {
    let store = SessionStore::with_config_dir(config_dir)?;
    let state = match store.state(session_id) {
        Ok(state) => state,
        Err(error)
            if error.contains("session state is missing")
                || error.contains("invalid state JSON") =>
        {
            crate::types::SessionState {
                status: Status::Idle,
                pid: None,
                run_started_at: None,
                last_run_at: None,
                last_message: None,
                hooks: None,
                token_estimate: None,
                restrictions: None,
            }
        }
        Err(error) => return Err(error),
    };
    let updates = SessionUpdate {
        status: Some(Status::Idle),
        pid: Some(None),
        run_started_at: Some(None),
        ..Default::default()
    };
    if store.state(session_id).is_err() {
        let path = store.state_path(session_id);
        let json = serde_json::to_string_pretty(&state)
            .map_err(|e| format!("failed to serialize recovered state: {}", e))?;
        let temp = path.with_extension("json.recovery.tmp");
        std::fs::write(&temp, json)
            .map_err(|e| format!("failed to write recovered state: {}", e))?;
        std::fs::rename(&temp, &path)
            .map_err(|e| format!("failed to install recovered state: {}", e))?;
    }
    store.update(session_id, updates)?;
    Ok(())
}

pub fn detect_crashed(session_id: &str, config_dir: &Path) -> Result<bool, String> {
    let store = SessionStore::with_config_dir(config_dir)?;
    let state = match store.state(session_id) {
        Ok(state) => state,
        Err(error)
            if error.contains("session state is missing")
                || error.contains("invalid state JSON") =>
        {
            return Ok(true);
        }
        Err(error) => return Err(error),
    };

    match (state.status, state.pid) {
        (Status::Running, Some(stored_pid)) => {
            if stored_pid == 0 || stored_pid > i32::MAX as u32 {
                return Ok(true);
            }
            #[cfg(unix)]
            {
                use nix::sys::signal;
                use nix::unistd::Pid;
                let pid = Pid::from_raw(stored_pid as i32);
                let is_alive = signal::kill(pid, None).is_ok();
                Ok(!is_alive)
            }
            #[cfg(not(unix))]
            {
                Ok(false)
            }
        }
        _ => Ok(false),
    }
}
