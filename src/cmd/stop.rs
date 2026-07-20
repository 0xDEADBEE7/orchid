use crate::session::{resolve, SessionStore};
use crate::types::Status;
use serde_json::json;
use std::path::Path;

pub fn stop(id: String, config_dir: &Path) -> Result<serde_json::Value, String> {
    stop_impl(&id, false, config_dir)
}

fn stop_impl(id: &str, force: bool, config_dir: &Path) -> Result<serde_json::Value, String> {
    let store = SessionStore::with_config_dir(config_dir)?;
    let base_path = config_dir.join("sessions");
    let meta = resolve::resolve(id, &base_path)?;
    let session_id = meta.id;

    let state = store.state(&session_id)?;
    if state.status == Status::Idle {
        return Ok(json!({
            "id": session_id,
            "status": "idle",
            "message": "session is not running"
        }));
    }

    if let Some(pid) = store.state(&session_id)?.pid {
        #[cfg(unix)]
        {
            use nix::sys::signal::{self, Signal};
            use nix::unistd::Pid;

            let pid = Pid::from_raw(pid as i32);
            let sig = if force {
                Signal::SIGKILL
            } else {
                Signal::SIGTERM
            };
            match signal::kill(pid, Some(sig)) {
                Ok(()) => {}
                Err(nix::Error::ESRCH) => {}
                Err(e) => return Err(format!("failed to send signal: {}", e)),
            }
        }

        #[cfg(not(unix))]
        let _ = pid;
    }

    #[cfg(unix)]
    if !force {
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    store.update(
        &session_id,
        crate::session::SessionUpdate {
            status: Some(Status::Idle),
            pid: Some(None),
            run_started_at: Some(None),
            ..Default::default()
        },
    )?;

    Ok(json!({
        "id": session_id,
        "status": "stopped",
        "killed": true
    }))
}
