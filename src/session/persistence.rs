use crate::types::{Metadata, SessionState};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) struct SessionPersistence {
    base_path: PathBuf,
}

impl SessionPersistence {
    pub(crate) fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    fn session_path(&self, id: &str) -> PathBuf {
        self.base_path.join(id)
    }

    fn metadata_path(&self, id: &str) -> PathBuf {
        self.session_path(id).join("metadata.json")
    }

    fn state_path(&self, id: &str) -> PathBuf {
        self.session_path(id).join("state.json")
    }

    pub(crate) fn read_state(&self, id: &str) -> Result<SessionState, String> {
        let path = self.state_path(id);
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(format!("session state is missing: {}", id));
            }
            Err(error) => return Err(format!("failed to read state: {}", error)),
        };
        serde_json::from_str(&contents).map_err(|e| format!("invalid state JSON: {}", e))
    }

    pub(crate) fn read_metadata(&self, id: &str) -> Result<Metadata, String> {
        let contents = fs::read_to_string(self.metadata_path(id))
            .map_err(|e| format!("failed to read metadata: {}", e))?;
        serde_json::from_str(&contents).map_err(|e| format!("invalid metadata JSON: {}", e))
    }

    pub(crate) fn update(
        &self,
        id: &str,
        updates: &super::SessionUpdate,
    ) -> Result<Metadata, String> {
        let mut state = self.read_state(id)?;
        if let Some(status) = &updates.status {
            state.status = status.clone();
        }
        if let Some(pid) = updates.pid {
            state.pid = pid;
        }
        if let Some(value) = updates.run_started_at {
            state.run_started_at = value;
        }
        if let Some(value) = updates.last_run_at {
            state.last_run_at = value;
        }
        if let Some(value) = &updates.last_message {
            state.last_message = Some(value.clone());
        }
        if let Some(value) = updates.token_estimate {
            state.token_estimate = Some(value);
        }
        if let Some(value) = &updates.restrictions {
            state.restrictions = value.clone();
        }
        self.write_state(id, &state)?;
        let mut meta = self.read_metadata(id)?;
        if let Some(value) = &updates.policy {
            meta.policy = value.clone();
        }
        if let Some(value) = &updates.policy_hash {
            meta.policy_hash = value.clone();
        }
        if let Some(value) = &updates.prompt {
            meta.prompt = value.clone();
        }
        if let Some(value) = &updates.label {
            meta.label = value.clone();
        }
        if let Some(value) = &updates.working_dir {
            meta.working_dir = value.clone();
        }
        meta.updated_at = chrono::Utc::now();
        self.write_metadata(id, &meta)?;
        Ok(meta)
    }

    pub(crate) fn write_state(&self, id: &str, state: &SessionState) -> Result<(), String> {
        self.write_json_atomically(
            &self.state_path(id),
            &self.session_path(id).join(".state.json.tmp"),
            state,
            "state",
        )
    }

    pub(crate) fn write_metadata(&self, id: &str, meta: &Metadata) -> Result<(), String> {
        self.write_json_atomically(
            &self.metadata_path(id),
            &self.session_path(id).join(".metadata.json.tmp"),
            meta,
            "metadata",
        )
    }

    fn write_json_atomically<T: serde::Serialize>(
        &self,
        path: &Path,
        temp: &Path,
        value: &T,
        name: &str,
    ) -> Result<(), String> {
        let json = serde_json::to_string_pretty(value)
            .map_err(|e| format!("failed to serialize {}: {}", name, e))?;
        fs::write(temp, json).map_err(|e| format!("failed to write temp {}: {}", name, e))?;
        fs::rename(temp, path).map_err(|e| format!("failed to rename {} file: {}", name, e))
    }
}
