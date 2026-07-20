use crate::convo::{MetadataUpdate, Store};
use crate::types::Status;
use chrono::Utc;
use std::process;
pub fn on_run_start(convo_id: &str) -> Result<(), String> {
    let store = Store::new()?;

    let updates = MetadataUpdate {
        status: Some(Status::Running),
        pid: Some(Some(process::id())),
        run_started_at: Some(Some(Utc::now())),
        ..Default::default()
    };

    store.update(convo_id, updates)?;
    Ok(())
}

pub fn on_run_end(convo_id: &str) -> Result<(), String> {
    let store = Store::new()?;

    let updates = MetadataUpdate {
        status: Some(Status::Idle),
        pid: Some(None),
        run_started_at: Some(None),
        last_run_at: Some(Some(Utc::now())),
        ..Default::default()
    };

    store.update(convo_id, updates)?;
    Ok(())
}

pub fn reconcile_crashed(convo_id: &str) -> Result<(), String> {
    let store = Store::new()?;
    let updates = MetadataUpdate {
        status: Some(Status::Idle),
        pid: Some(None),
        run_started_at: Some(None),
        ..Default::default()
    };
    store.update(convo_id, updates)?;
    Ok(())
}

pub fn detect_crashed(convo_id: &str) -> Result<bool, String> {
    let store = Store::new()?;
    let meta = store.get(convo_id)?;

    match (meta.status, meta.pid) {
        (Status::Running, Some(stored_pid)) => {
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
