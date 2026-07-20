use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub mod resolve;

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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RootConfig {
    pub policy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Connection {
    pub interface: String,
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    pub model: String,
    #[serde(default)]
    pub params: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Permissions {
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PolicyLimits {
    #[serde(default)]
    pub token_warn_threshold: Option<u32>,
    #[serde(default)]
    pub token_hard_limit: Option<u32>,
    #[serde(default)]
    pub max_steps: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Policy {
    pub connections: Vec<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub permissions: Permissions,
    #[serde(default)]
    pub limits: PolicyLimits,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug)]
pub enum ResourceLoadError {
    Missing {
        kind: &'static str,
        path: PathBuf,
    },
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    Invalid {
        path: PathBuf,
        message: String,
    },
}

impl std::fmt::Display for ResourceLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing { kind, path } => write!(f, "missing {} at {}", kind, path.display()),
            Self::Read { path, source } => {
                write!(f, "failed to read {}: {}", path.display(), source)
            }
            Self::Parse { path, source } => {
                write!(f, "invalid JSON at {}: {}", path.display(), source)
            }
            Self::Invalid { path, message } => {
                write!(f, "invalid resource at {}: {}", path.display(), message)
            }
        }
    }
}
impl std::error::Error for ResourceLoadError {}

fn read_json<T: serde::de::DeserializeOwned>(
    path: PathBuf,
    kind: &'static str,
) -> Result<T, ResourceLoadError> {
    if !path.is_file() {
        return Err(ResourceLoadError::Missing { kind, path });
    }
    let contents = fs::read_to_string(&path).map_err(|source| ResourceLoadError::Read {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(&contents).map_err(|source| ResourceLoadError::Parse { path, source })
}

fn resource_name(name: &str) -> Result<(), String> {
    let valid = !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..");
    if valid {
        Ok(())
    } else {
        Err("resource name must be a non-empty path component without '..'".to_string())
    }
}

impl ConfigDir {
    pub fn load_root(&self) -> Result<RootConfig, ResourceLoadError> {
        let root: RootConfig = read_json(self.root_path(), "root config")?;
        if root.policy.trim().is_empty() {
            return Err(ResourceLoadError::Invalid {
                path: self.root_path(),
                message: "policy must be a non-empty resource name".into(),
            });
        }
        resource_name(&root.policy).map_err(|message| ResourceLoadError::Invalid {
            path: self.root_path(),
            message,
        })?;
        Ok(root)
    }
    pub fn load_connection(&self, name: &str) -> Result<Connection, ResourceLoadError> {
        resource_name(name).map_err(|message| ResourceLoadError::Invalid {
            path: self.connections_path().join(format!("{}.json", name)),
            message,
        })?;
        let path = self.connections_path().join(format!("{}.json", name));
        let value: Connection = read_json(path.clone(), "connection")?;
        if value.interface.trim().is_empty()
            || value.base_url.trim().is_empty()
            || value.model.trim().is_empty()
        {
            return Err(ResourceLoadError::Invalid {
                path,
                message: "interface, base_url, and model are required".into(),
            });
        }
        Ok(value)
    }
    pub fn load_policy(&self, name: &str) -> Result<Policy, ResourceLoadError> {
        resource_name(name).map_err(|message| ResourceLoadError::Invalid {
            path: self.policies_path().join(format!("{}.json", name)),
            message,
        })?;
        let path = self.policies_path().join(format!("{}.json", name));
        let value: Policy = read_json(path.clone(), "policy")?;
        if value.connections.is_empty() {
            return Err(ResourceLoadError::Invalid {
                path,
                message: "connections must not be empty".into(),
            });
        }
        for connection in &value.connections {
            self.load_connection(connection)
                .map_err(|e| ResourceLoadError::Invalid {
                    path: path.clone(),
                    message: e.to_string(),
                })?;
        }
        if let Some(prompt) = &value.prompt {
            self.load_prompt(prompt)
                .map_err(|e| ResourceLoadError::Invalid {
                    path: path.clone(),
                    message: e.to_string(),
                })?;
        }
        Ok(value)
    }
    pub fn load_prompt(&self, name: &str) -> Result<String, ResourceLoadError> {
        let path = self.prompts_path().join(format!("{}.md", name));
        resource_name(name).map_err(|message| ResourceLoadError::Invalid {
            path: path.clone(),
            message,
        })?;
        if !path.is_file() {
            return Err(ResourceLoadError::Missing {
                kind: "prompt",
                path,
            });
        }
        fs::read_to_string(&path).map_err(|source| ResourceLoadError::Read { path, source })
    }
    pub fn validate(&self) -> Result<(), ResourceLoadError> {
        let root = self.load_root()?;
        self.load_policy(&root.policy).map(|_| ())
    }
}
