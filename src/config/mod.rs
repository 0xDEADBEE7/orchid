use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigDir(PathBuf);

impl ConfigDir {
    pub fn new(path: impl Into<PathBuf>) -> Self { Self(path.into()) }
    pub fn path(&self) -> &Path { &self.0 }
    pub fn root_path(&self) -> PathBuf { self.0.join("config.json") }
    pub fn connections_path(&self) -> PathBuf { self.0.join("connections") }
    pub fn policies_path(&self) -> PathBuf { self.0.join("policies") }
    pub fn prompts_path(&self) -> PathBuf { self.0.join("prompts") }
    pub fn sessions_path(&self) -> PathBuf { self.0.join("sessions") }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RootConfig { pub policy: String }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Connection {
    pub interface: String,
    pub base_url: String,
    #[serde(default)] pub api_key: Option<String>,
    pub model: String,
    #[serde(default)] pub params: HashMap<String, serde_json::Value>,
    #[serde(default)] pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Permissions {
    #[serde(default)] pub tools: Vec<String>,
    #[serde(default)] pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PolicyLimits {
    #[serde(default)] pub token_warn_threshold: Option<u32>,
    #[serde(default)] pub token_hard_limit: Option<u32>,
    #[serde(default)] pub max_steps: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Policy {
    pub connections: Vec<String>,
    #[serde(default)] pub prompt: Option<String>,
    #[serde(default)] pub permissions: Permissions,
    #[serde(default)] pub limits: PolicyLimits,
}

#[derive(Debug)]
pub enum ResourceLoadError {
    Missing { kind: &'static str, path: PathBuf },
    Read { path: PathBuf, source: std::io::Error },
    Parse { path: PathBuf, source: serde_json::Error },
    Invalid { path: PathBuf, message: String },
}

impl std::fmt::Display for ResourceLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing { kind, path } => write!(f, "missing {} at {}", kind, path.display()),
            Self::Read { path, source } => write!(f, "failed to read {}: {}", path.display(), source),
            Self::Parse { path, source } => write!(f, "invalid JSON at {}: {}", path.display(), source),
            Self::Invalid { path, message } => write!(f, "invalid resource at {}: {}", path.display(), message),
        }
    }
}
impl std::error::Error for ResourceLoadError {}

fn read_json<T: serde::de::DeserializeOwned>(path: PathBuf, kind: &'static str) -> Result<T, ResourceLoadError> {
    if !path.is_file() { return Err(ResourceLoadError::Missing { kind, path }); }
    let contents = fs::read_to_string(&path).map_err(|source| ResourceLoadError::Read { path: path.clone(), source })?;
    serde_json::from_str(&contents).map_err(|source| ResourceLoadError::Parse { path, source })
}

fn resource_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err("resource name must be a non-empty path component".to_string());
    }
    Ok(())
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
        let path = self.connections_path().join(format!("{}.json", name));
        resource_name(name).map_err(|message| ResourceLoadError::Invalid { path: path.clone(), message })?;
        let value: Connection = read_json(path.clone(), "connection")?;
        if value.interface.trim().is_empty() || value.base_url.trim().is_empty() || value.model.trim().is_empty() {
            return Err(ResourceLoadError::Invalid { path, message: "interface, base_url, and model are required".into() });
        }
        Ok(value)
    }
    pub fn load_policy(&self, name: &str) -> Result<Policy, ResourceLoadError> {
        let path = self.policies_path().join(format!("{}.json", name));
        resource_name(name).map_err(|message| ResourceLoadError::Invalid { path: path.clone(), message })?;
        let value: Policy = read_json(path.clone(), "policy")?;
        if value.connections.is_empty() { return Err(ResourceLoadError::Invalid { path, message: "connections must not be empty".into() }); }
        for connection in &value.connections { self.load_connection(connection).map_err(|e| ResourceLoadError::Invalid { path: path.clone(), message: e.to_string() })?; }
        if let Some(prompt) = &value.prompt { self.load_prompt(prompt).map_err(|e| ResourceLoadError::Invalid { path: path.clone(), message: e.to_string() })?; }
        Ok(value)
    }
    pub fn load_prompt(&self, name: &str) -> Result<String, ResourceLoadError> {
        let path = self.prompts_path().join(format!("{}.md", name));
        resource_name(name).map_err(|message| ResourceLoadError::Invalid { path: path.clone(), message })?;
        if !path.is_file() { return Err(ResourceLoadError::Missing { kind: "prompt", path }); }
        fs::read_to_string(&path).map_err(|source| ResourceLoadError::Read { path, source })
    }
    pub fn validate(&self) -> Result<(), ResourceLoadError> {
        let root = self.load_root()?;
        self.load_policy(&root.policy).map(|_| ())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    // `name` is the map key, not a field in the JSON object — make optional
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub provider: String,
    // Flexible: api_key, base_url, model etc. are profile-specific but unused here
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub model: String,
    /// Arbitrary request body parameters (e.g. `max_completion_tokens`, `temperature`,
    /// `max_tokens`).
    #[serde(default)]
    pub params: HashMap<String, serde_json::Value>,
    /// Arbitrary headers injected into every request. Values support `env.<VAR>` indirection.
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Optional server-side management actions (list/load/unload models, etc.).
    /// Declared in profile config; executed by `orchid server-action`.
    #[serde(default)]
    pub server_actions: HashMap<String, ServerAction>,
    #[serde(flatten, default)]
    pub extra: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// A server-side action descriptor declared in a profile's config.
///
/// The CLI executes the HTTP request; downstream clients (Emacs, scripts)
/// invoke through `orchid server-action`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerAction {
    /// HTTP method: GET, POST, PUT, DELETE
    pub method: String,
    /// Relative path appended to profile's `base_url`
    pub path: String,
    /// Keys the caller may pass via `--key value` flags. Each becomes
    /// a field in the JSON body. Omitted if not supplied.
    #[serde(default)]
    pub body_params: Vec<String>,
    /// Extra headers beyond auth. Values support `env.` resolution.
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

/// Per-session resource limits. All fields optional; unset values use hardcoded defaults.
///
/// Example config.json:
/// ```json
/// {
///   "limits": {
///     "token_warn_threshold": 80000,
///     "token_hard_limit": 120000
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Limits {
    /// Token count at which a warning system message is injected. Default: 80,000.
    pub token_warn_threshold: Option<u32>,
    /// Token count at which the run is terminated. Default: 120,000.
    pub token_hard_limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // Support both "current_profile" (old schema) and "active_profile" (current schema)
    #[serde(alias = "active_profile")]
    pub current_profile: Option<String>,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
    #[serde(default)]
    pub limits: Limits,
    /// Glob patterns for paths universally allowed outside the working directory.
    /// Checked before per-conversation exceptions.
    #[serde(default)]
    pub scope_exceptions: Vec<String>,
    /// Diagnostic log verbosity: "debug" enables debug-level events in orchid.log.
    /// Omit or set to "info" for default behaviour.
    pub log_level: Option<String>,
    /// Path to a shell env file to source into bash tool executions (e.g. `~/.config/orchid/env`).
    pub env_file: Option<String>,
    // Ignore extra top-level keys (personas, etc.)
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Resolve orchid config directory with XDG standard support.
///
/// Priority (in order):
/// 1. ORCHID_DIR env var (explicit override)
/// 2. XDG_CONFIG_HOME env var (user XDG preference)
/// 3. $HOME/.config (XDG standard)
/// 4. dirs::config_dir().join("orchid") (platform-specific fallback)
pub fn get_orchid_dir() -> Result<PathBuf, String> {
    if let Ok(orchid_dir) = env::var("ORCHID_DIR") {
        return Ok(PathBuf::from(orchid_dir));
    }

    if let Ok(xdg_config_home) = env::var("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(xdg_config_home).join("orchid"));
    }

    if let Ok(home) = env::var("HOME") {
        return Ok(PathBuf::from(home).join(".config").join("orchid"));
    }

    dirs::config_dir()
        .map(|p| p.join("orchid"))
        .ok_or_else(|| "could not determine config directory".to_string())
}

pub fn load_config() -> Result<Config, String> {
    let config_path = config_path()?;

    if !config_path.exists() {
        return Err(format!("config not found at {}", config_path.display()));
    }

    let contents =
        fs::read_to_string(&config_path).map_err(|e| format!("failed to read config: {}", e))?;

    serde_json::from_str(&contents).map_err(|e| format!("invalid config JSON: {}", e))
}

fn config_path() -> Result<PathBuf, String> {
    get_orchid_dir().map(|p| p.join("config.json"))
}

/// Parse a shell env file into key-value pairs.
/// Supports `KEY=VALUE` and `export KEY=VALUE`. Ignores blank lines and comments.
pub fn load_env_file(path: &str) -> HashMap<String, String> {
    let expanded = path.replacen('~', &env::var("HOME").unwrap_or_default(), 1);
    let contents = match fs::read_to_string(&expanded) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    let mut vars = HashMap::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        // Strip optional surrounding quotes from value
        if let Some((key, val)) = line.split_once('=') {
            let val = val.trim_matches('"').trim_matches('\'');
            vars.insert(key.trim().to_string(), val.to_string());
        }
    }
    vars
}

pub fn resolve_env(profile: &Profile) -> HashMap<String, String> {
    let mut resolved = HashMap::new();

    for (key, value) in &profile.env {
        let resolved_value = if let Some(env_var) = value.strip_prefix("env.") {
            env::var(env_var).unwrap_or_else(|_| String::new())
        } else {
            value.clone()
        };

        resolved.insert(key.clone(), resolved_value);
    }

    resolved
}


