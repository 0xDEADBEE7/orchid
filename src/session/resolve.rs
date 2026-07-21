use crate::types::Metadata;
use std::path::{Path, PathBuf};

pub struct SessionStore {
    base_path: PathBuf,
}

impl SessionStore {
    pub fn new(config_dir: impl Into<PathBuf>) -> Result<Self, String> {
        let base_path = config_dir.into().join("sessions");
        std::fs::create_dir_all(&base_path)
            .map_err(|e| format!("failed to create sessions dir: {}", e))?;
        Ok(Self { base_path })
    }

    pub fn metadata(&self, id: &str) -> Result<Metadata, String> {
        let path = self.base_path.join(id).join("metadata.json");
        let contents = std::fs::read_to_string(&path)
            .map_err(|e| format!("failed to read metadata: {}", e))?;
        serde_json::from_str(&contents).map_err(|e| format!("invalid metadata JSON: {}", e))
    }

    pub fn state(&self, id: &str) -> Result<serde_json::Value, String> {
        let path = self.base_path.join(id).join("state.json");
        let contents =
            std::fs::read_to_string(&path).map_err(|e| format!("failed to read state: {}", e))?;
        serde_json::from_str(&contents).map_err(|e| format!("invalid state JSON: {}", e))
    }
}
pub fn resolve(id: &str, base_path: &std::path::Path) -> Result<Metadata, String> {
    if !is_id_format(id) {
        return Err(format!(
            "invalid session ID: '{}' (must be 32 hex characters)",
            id
        ));
    }
    read_metadata(id, base_path)
}

pub fn is_id_format(s: &str) -> bool {
    s.len() == 32 && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn read_metadata(id: &str, base_path: &Path) -> Result<Metadata, String> {
    let metadata_path = base_path.join(id).join("metadata.json");
    let contents = std::fs::read_to_string(&metadata_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!("session not found: {}", id)
        } else {
            format!("failed to read metadata: {}", e)
        }
    })?;
    serde_json::from_str(&contents).map_err(|e| format!("invalid metadata JSON: {}", e))
}
