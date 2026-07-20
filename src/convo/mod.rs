use crate::get_orchid_dir;
use crate::types::{Metadata, Status};
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};

pub mod id;
pub mod resolve;

pub use id::generate_id;
pub use resolve::{is_id_format, resolve};

pub struct Store {
    base_path: PathBuf,
}

impl Store {
    pub fn new() -> Result<Self, String> {
        let base_path = get_orchid_dir()?.join("conversations");

        fs::create_dir_all(&base_path)
            .map_err(|e| format!("failed to create conversations dir: {}", e))?;

        Ok(Store { base_path })
    }

    /// Create a store rooted under a config directory's sessions path.
    /// This is the new-path that stores sessions under `./config/sessions/`.
    pub fn with_config_dir(config_dir: &Path) -> Result<Self, String> {
        let base_path = config_dir.join("sessions");

        fs::create_dir_all(&base_path)
            .map_err(|e| format!("failed to create sessions dir: {}", e))?;

        Ok(Store { base_path })
    }

    pub fn with_base(base_path: PathBuf) -> Self {
        Store { base_path }
    }

    /// Get the base path for this store (for tests).
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    pub fn create(
        &self,
        label: Option<String>,
        working_dir: Option<String>,
        persona: Option<String>,
        _profile: Option<String>,
        scope_exceptions: Option<Vec<String>>,
    ) -> Result<Metadata, String> {
        loop {
            let id = id::generate_id();

            if !id::exists_check(&id, &self.base_path) {
                let convo_dir = self.base_path.join(&id);
                fs::create_dir_all(&convo_dir)
                    .map_err(|e| format!("failed to create conversation dir: {}", e))?;

                let now = Utc::now();
                let meta = Metadata {
                    id: id.clone(),
                    label,
                    persona,
                    working_dir,
                    env: None,
                    created_at: now,
                    updated_at: now,
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
                return Ok(meta);
            }
        }
    }

    pub fn get(&self, id: &str) -> Result<Metadata, String> {
        let metadata_path = self.base_path.join(id).join("metadata.json");
        let contents = fs::read_to_string(&metadata_path)
            .map_err(|e| format!("failed to read metadata: {}", e))?;
        serde_json::from_str(&contents).map_err(|e| format!("invalid metadata JSON: {}", e))
    }

    pub fn list(&self) -> Result<Vec<Metadata>, String> {
        let mut convos = Vec::new();
        let entries = fs::read_dir(&self.base_path)
            .map_err(|e| format!("failed to read conversations dir: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("dir entry error: {}", e))?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Ok(meta) = self.get(dir_name) {
                        convos.push(meta);
                    }
                }
            }
        }

        convos.sort_by(|a, b| {
            let a_time = a.run_started_at.unwrap_or(a.created_at);
            let b_time = b.run_started_at.unwrap_or(b.created_at);
            b_time.cmp(&a_time)
        });

        Ok(convos)
    }

    pub fn update(&self, id: &str, updates: MetadataUpdate) -> Result<Metadata, String> {
        let mut meta = self.get(id)?;

        if let Some(label) = updates.label {
            meta.label = label;
        }
        if let Some(persona) = updates.persona {
            meta.persona = persona;
        }
        if let Some(working_dir) = updates.working_dir {
            meta.working_dir = working_dir;
        }
        if let Some(status) = updates.status {
            meta.status = status;
        }
        if let Some(pid) = updates.pid {
            meta.pid = pid;
        }
        if let Some(run_started_at) = updates.run_started_at {
            meta.run_started_at = run_started_at;
        }
        if let Some(last_run_at) = updates.last_run_at {
            meta.last_run_at = last_run_at;
        }
        if let Some(last_message) = updates.last_message {
            meta.last_message = Some(last_message);
        }
        if let Some(token_estimate) = updates.token_estimate {
            meta.token_estimate = Some(token_estimate);
        }
        if let Some(scope_exceptions) = updates.scope_exceptions {
            meta.scope_exceptions = scope_exceptions;
        }

        meta.updated_at = Utc::now();

        self.write_metadata(id, &meta)?;
        Ok(meta)
    }

    fn write_metadata(&self, id: &str, meta: &Metadata) -> Result<(), String> {
        let metadata_path = self.base_path.join(id).join("metadata.json");
        let temp_path = self.base_path.join(id).join(".metadata.json.tmp");

        let json = serde_json::to_string_pretty(meta)
            .map_err(|e| format!("failed to serialize metadata: {}", e))?;

        fs::write(&temp_path, &json)
            .map_err(|e| format!("failed to write temp metadata: {}", e))?;

        fs::rename(&temp_path, &metadata_path)
            .map_err(|e| format!("failed to rename metadata file: {}", e))?;

        Ok(())
    }
}

#[derive(Default)]
pub struct MetadataUpdate {
    pub label: Option<Option<String>>,
    pub persona: Option<Option<String>>,
    pub working_dir: Option<Option<String>>,
    pub status: Option<Status>,
    pub pid: Option<Option<u32>>,
    pub run_started_at: Option<Option<chrono::DateTime<chrono::Utc>>>,
    pub last_run_at: Option<Option<chrono::DateTime<chrono::Utc>>>,
    pub last_message: Option<String>,
    pub token_estimate: Option<u32>,
    pub scope_exceptions: Option<Option<Vec<String>>>,
}

/// Helper to resolve convo.jsonl path with XDG support.
pub fn get_convo_jsonl_path(convo_id: &str) -> Result<PathBuf, String> {
    let base_path = get_orchid_dir()?.join("conversations").join(convo_id);

    Ok(base_path.join("conversation.jsonl"))
}

/// Get the conversation directory under the default XDG location.
pub fn get_convo_dir(convo_id: &str) -> Result<PathBuf, String> {
    let base_path = get_orchid_dir()?.join("conversations").join(convo_id);

    Ok(base_path)
}

/// Get the conversation directory under a config directory's sessions path.
pub fn get_convo_dir_from_config(convo_id: &str, config_dir: &Path) -> Result<PathBuf, String> {
    let base_path = config_dir.join("sessions").join(convo_id);

    Ok(base_path)
}
