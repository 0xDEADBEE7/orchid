use std::path::{Path, PathBuf};

/// Root of a complete, self-contained Orchid configuration tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigDir(PathBuf);

impl ConfigDir {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn root_path(&self) -> PathBuf {
        self.0.join("config.json")
    }

    pub fn connections_path(&self) -> PathBuf {
        self.0.join("connections")
    }

    pub fn policies_path(&self) -> PathBuf {
        self.0.join("policies")
    }

    pub fn prompts_path(&self) -> PathBuf {
        self.0.join("prompts")
    }

    pub fn sessions_path(&self) -> PathBuf {
        self.0.join("sessions")
    }

    pub fn auth_path(&self) -> PathBuf {
        self.0.join("auth")
    }
}
