use crate::session::{is_id_format, SessionStore};
use crate::types::Status;
use serde_json::json;
use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, Instant};

pub fn await_sessions(
    ids: Vec<String>,
    timeout: f64,
    interval: f64,
    config_dir: &Path,
) -> Result<(serde_json::Value, i32), String> {
    let mut unique = Vec::new();
    let mut seen = HashSet::new();
    for id in ids {
        if !is_id_format(&id) {
            return Err(format!("invalid session ID: '{}'", id));
        }
        if seen.insert(id.clone()) {
            unique.push(id);
        }
    }
    let store = SessionStore::with_config_dir(config_dir)?;
    let deadline = Instant::now() + Duration::from_secs_f64(timeout);
    loop {
        let mut completed = Vec::new();
        for id in &unique {
            let state = store.state(id)?;
            if matches!(
                state.status,
                Status::Idle | Status::Failed | Status::Cancelled
            ) {
                completed.push(json!({"id": id, "status": state.status}));
            }
        }
        if !completed.is_empty() {
            return Ok((json!({"completed": completed}), 0));
        }
        if Instant::now() >= deadline {
            return Ok((json!({"completed": [], "timed_out": true}), 2));
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        std::thread::sleep(Duration::from_secs_f64(interval).min(remaining));
    }
}
