use crate::types::{Metadata, SessionState, Status};
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};

pub mod id;
pub mod resolve;

pub use id::generate_id;
pub use resolve::{is_id_format, resolve};

pub struct SessionStore {
    base_path: PathBuf,
}

impl SessionStore {
    pub fn new() -> Result<Self, String> {
        Self::with_config_dir(Path::new("config"))
    }

    /// Create a store rooted under a config directory's sessions path.
    /// This is the new-path that stores sessions under `./config/sessions/`.
    pub fn with_config_dir(config_dir: &Path) -> Result<Self, String> {
        let base_path = config_dir.join("sessions");

        fs::create_dir_all(&base_path)
            .map_err(|e| format!("failed to create sessions dir: {}", e))?;

        Ok(SessionStore { base_path })
    }

    pub fn with_base(base_path: PathBuf) -> Self {
        SessionStore { base_path }
    }

    /// Get the base path for this store (for tests).
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    pub fn session_path(&self, id: &str) -> PathBuf {
        self.base_path.join(id)
    }

    pub fn transcript_path(&self, id: &str) -> PathBuf {
        self.session_path(id).join("conversation.jsonl")
    }

    pub fn metadata_path(&self, id: &str) -> PathBuf {
        self.session_path(id).join("metadata.json")
    }

    pub fn state_path(&self, id: &str) -> PathBuf {
        self.session_path(id).join("state.json")
    }

    pub fn update_for_config(
        &self,
        config_dir: &Path,
        id: &str,
        updates: SessionUpdate,
    ) -> Result<Metadata, String> {
        SessionStore::with_config_dir(config_dir)?.update(id, updates)
    }

    pub fn create(
        &self,
        label: Option<String>,
        working_dir: Option<String>,
        scope_exceptions: Option<Vec<String>>,
    ) -> Result<Metadata, String> {
        loop {
            let id = id::generate_id();

            if !id::exists_check(&id, &self.base_path) {
                let session_dir = self.base_path.join(&id);
                fs::create_dir_all(&session_dir)
                    .map_err(|e| format!("failed to create session directory: {}", e))?;

                let now = Utc::now();
                let meta = Metadata {
                    id: id.clone(),
                    policy: None,
                    policy_hash: None,
                    label,
                    working_dir,
                    env: None,
                    created_at: now,
                    updated_at: now,
                };
                let state = SessionState {
                    status: Status::Idle,
                    pid: None,
                    run_started_at: None,
                    last_run_at: None,
                    last_message: None,
                    hooks: None,
                    token_estimate: None,
                    allow_scope_escape: None,
                    scope_exceptions,
                };

                self.write_metadata(&id, &meta)?;
                self.write_state(&id, &state)?;
                return Ok(meta);
            }
        }
    }
    pub fn state(&self, id: &str) -> Result<SessionState, String> {
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

    pub fn status(&self, id: &str) -> Result<Status, String> {
        Ok(self.state(id)?.status)
    }

    pub fn pid(&self, id: &str) -> Result<Option<u32>, String> {
        Ok(self.state(id)?.pid)
    }

    fn write_state(&self, id: &str, state: &SessionState) -> Result<(), String> {
        self.write_json_atomically(
            &self.state_path(id),
            &self.session_path(id).join(".state.json.tmp"),
            state,
            "state",
        )
    }
    pub fn get(&self, id: &str) -> Result<Metadata, String> {
        let metadata_path = self.metadata_path(id);
        let contents = fs::read_to_string(&metadata_path)
            .map_err(|e| format!("failed to read metadata: {}", e))?;
        serde_json::from_str(&contents).map_err(|e| format!("invalid metadata JSON: {}", e))
    }

    pub fn list(&self) -> Result<Vec<Metadata>, String> {
        let mut sessions = Vec::new();
        let entries = fs::read_dir(&self.base_path)
            .map_err(|e| format!("failed to read sessions dir: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("session entry error: {}", e))?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if dir_name.starts_with('.') || !is_id_format(dir_name) {
                continue;
            }
            if let Ok(meta) = self.get(dir_name) {
                sessions.push(meta);
            }
        }
        sessions.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(sessions)
    }

    pub fn update(&self, id: &str, updates: SessionUpdate) -> Result<Metadata, String> {
        let mut state = self.state(id)?;
        if let Some(status) = updates.status {
            state.status = status;
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
        if let Some(value) = updates.last_message {
            state.last_message = Some(value);
        }
        if let Some(value) = updates.token_estimate {
            state.token_estimate = Some(value);
        }
        if let Some(value) = updates.scope_exceptions {
            state.scope_exceptions = value;
        }
        self.write_state(id, &state)?;

        let mut meta = self.get(id)?;

        if let Some(policy) = updates.policy {
            meta.policy = policy;
        }
        if let Some(policy_hash) = updates.policy_hash {
            meta.policy_hash = policy_hash;
        }
        if let Some(label) = updates.label {
            meta.label = label;
        }
        if let Some(working_dir) = updates.working_dir {
            meta.working_dir = working_dir;
        }

        meta.updated_at = Utc::now();

        self.write_metadata(id, &meta)?;
        Ok(meta)
    }

    fn write_metadata(&self, id: &str, meta: &Metadata) -> Result<(), String> {
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

#[derive(Default)]
pub struct SessionUpdate {
    pub policy: Option<Option<String>>,
    pub policy_hash: Option<Option<String>>,
    pub label: Option<Option<String>>,
    pub working_dir: Option<Option<String>>,
    pub status: Option<Status>,
    pub pid: Option<Option<u32>>,
    pub run_started_at: Option<Option<chrono::DateTime<chrono::Utc>>>,
    pub last_run_at: Option<Option<chrono::DateTime<chrono::Utc>>>,
    pub last_message: Option<String>,
    pub token_estimate: Option<u32>,
    pub scope_exceptions: Option<Option<Vec<String>>>,
}

/// Resolve the default session transcript path.
pub fn get_session_jsonl_path(session_id: &str) -> Result<PathBuf, String> {
    let base_path = Path::new("config").join("sessions").join(session_id);

    Ok(base_path.join("conversation.jsonl"))
}

pub fn get_session_jsonl_path_from_config(
    session_id: &str,
    config_dir: &Path,
) -> Result<PathBuf, String> {
    if !is_id_format(session_id) {
        return Err(format!("invalid session ID: '{}'", session_id));
    }
    Ok(config_dir
        .join("sessions")
        .join(session_id)
        .join("conversation.jsonl"))
}

pub fn get_session_dir(session_id: &str) -> Result<PathBuf, String> {
    let base_path = Path::new("config").join("sessions").join(session_id);

    Ok(base_path)
}

/// Get the session directoryectory under a config directory's sessions path.
pub fn get_session_dir_from_config(session_id: &str, config_dir: &Path) -> Result<PathBuf, String> {
    let base_path = config_dir.join("sessions").join(session_id);

    Ok(base_path)
}
