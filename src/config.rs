//! Provides the config functionality.
use crate::{agent::Agent, model::Session};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

mod auth_command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Credential {
    ApiKey(String),
    Codex {
        access_token: String,
        account_id: String,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub log_level: Option<String>,
    /// Hooks that run for every agent.
    #[serde(default)]
    pub hooks: HookMap,
}

/// Hook scripts by event name, run in order.
pub type HookMap = HashMap<String, Vec<HookDefinition>>;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HookDefinition {
    pub script: String,
    /// Seconds before the hook is killed (default 30).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    pub interface: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub auth: Option<String>,
    #[serde(default)]
    pub params: HashMap<String, Value>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub root: PathBuf,
    pub log_level: String,
    /// Global hooks from config.json.
    pub hooks: HookMap,
    /// The agent in effect: the default agent, or a session's own copy.
    pub agent: Agent,
    /// Set when acting on a session, whose prompt.md is the system prompt.
    pub session: Option<String>,
}

impl Settings {
    /// Returns the system prompt in effect.
    pub fn prompt(&self) -> io::Result<String> {
        match &self.session {
            Some(id) => fs::read_to_string(self.root.join("sessions").join(id).join("prompt.md")),
            None => self.prompt_text(&self.agent.prompt),
        }
    }

    /// Reads `prompts/<name>.md`, or `prompts/<name>.txt`.
    pub fn prompt_text(&self, name: &str) -> io::Result<String> {
        let prompts = self.root.join("prompts");
        fs::read_to_string(prompts.join(format!("{name}.md")))
            .or_else(|_| fs::read_to_string(prompts.join(format!("{name}.txt"))))
            .map_err(|_| {
                io::Error::new(io::ErrorKind::NotFound, format!("prompt not found: {name}"))
            })
    }

    /// Hooks for an event: global hooks first, then the agent's own.
    pub fn hooks(&self, event: &str) -> impl Iterator<Item = &HookDefinition> {
        let global = self.hooks.get(event).into_iter().flatten();
        global.chain(self.agent.hooks.get(event).into_iter().flatten())
    }

    /// Settings for acting on a session: its own agent copy and prompt. Every
    /// path that touches a session uses this, so all of them see the same hooks.
    pub fn for_session(&self, session: &Session) -> Settings {
        Settings {
            agent: session.metadata.agent.clone(),
            session: Some(session.metadata.id.clone()),
            ..self.clone()
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedConnection {
    pub connection: Connection,
    pub credential: Option<Credential>,
    pub params: HashMap<String, Value>,
    pub headers: HashMap<String, String>,
}

impl Settings {
    /// Performs the connection operation.
    pub fn connection(&self, name: &str) -> io::Result<Connection> {
        read_json(&self.root.join("connections").join(format!("{name}.json")))
    }

    /// Performs the resolve connection operation.
    pub fn resolve_connection(&self, name: &str) -> io::Result<ResolvedConnection> {
        if name == "echo" {
            return Ok(ResolvedConnection {
                connection: echo_connection(),
                credential: None,
                params: HashMap::new(),
                headers: HashMap::new(),
            });
        }
        let connection = self.connection(name)?;
        validate_connection(&connection)?;
        let credential = self.resolve_credential(&connection)?;
        let headers = resolve_headers(&connection.headers)?;
        Ok(ResolvedConnection {
            params: connection.params.clone(),
            connection,
            credential,
            headers,
        })
    }

    /// Performs the resolve credential operation.
    fn resolve_credential(&self, connection: &Connection) -> io::Result<Option<Credential>> {
        if connection.interface == "local" {
            return Ok(None);
        }
        if let Some(auth) = connection.auth.as_deref() {
            return self.resolve_auth(auth);
        }
        connection
            .api_key
            .as_deref()
            .map(resolve_secret)
            .transpose()
            .map(|key| key.map(Credential::ApiKey))
    }

    /// Performs the resolve auth operation.
    fn resolve_auth(&self, name: &str) -> io::Result<Option<Credential>> {
        let profile = self.root.join("auth").join(format!("{name}.json"));
        if profile.exists() {
            let value: Value = read_json(&profile)?;
            let kind = value["type"].as_str().unwrap_or("api_key");
            if kind == "api_key" {
                let reference = value["value"]
                    .as_str()
                    .ok_or_else(|| safe_config_error("API key credential unavailable"))?;
                return Ok(Some(Credential::ApiKey(resolve_secret(reference)?)));
            }
        }
        Ok(Some(self.codex_credential(name)?))
    }

    /// Performs the codex credential operation.
    fn codex_credential(&self, name: &str) -> io::Result<Credential> {
        let tokens = crate::client::openai_codex::auth::access_token(&self.root, name)
            .map_err(|_| safe_config_error("Codex credential unavailable"))?;
        let access_token = tokens.access_token;
        let account_id = tokens.account_id;
        Ok(Credential::Codex {
            access_token,
            account_id,
        })
    }
}

/// Performs the echo connection operation.
fn echo_connection() -> Connection {
    Connection {
        interface: "echo".into(),
        base_url: String::new(),
        model: String::new(),
        api_key: None,
        auth: None,
        params: HashMap::new(),
        headers: HashMap::new(),
    }
}

/// Performs the validate connection operation.
fn validate_connection(connection: &Connection) -> io::Result<()> {
    if connection.interface.is_empty()
        || connection.base_url.is_empty()
        || connection.model.is_empty()
    {
        return Err(safe_config_error(
            "connection requires interface, base_url, and model",
        ));
    }
    if matches!(
        connection.interface.as_str(),
        "local" | "openai" | "codex" | "openai-codex" | "echo"
    ) {
        Ok(())
    } else {
        Err(safe_config_error("unknown connection interface"))
    }
}

/// Performs the resolve headers operation.
fn resolve_headers(headers: &HashMap<String, String>) -> io::Result<HashMap<String, String>> {
    headers
        .iter()
        .map(|(name, value)| resolve_inline(value).map(|value| (name.clone(), value)))
        .collect()
}
/// Performs the resolve inline operation.
fn resolve_inline(reference: &str) -> io::Result<String> {
    if reference.starts_with("env.") {
        return resolve_secret(reference);
    }
    Ok(reference.to_owned())
}

/// Performs the resolve secret operation.
fn resolve_secret(reference: &str) -> io::Result<String> {
    let value = if let Some(name) = reference.strip_prefix("env.") {
        std::env::var(name).map_err(|_| safe_config_error("required credential is unavailable"))?
    } else if let Some(path) = reference.strip_prefix("file.") {
        fs::read_to_string(path)
            .map_err(|_| safe_config_error("required credential is unavailable"))?
    } else {
        return Err(safe_config_error("invalid credential reference"));
    };
    let value = value.trim_end_matches('\n').to_owned();
    if value.is_empty() {
        Err(safe_config_error("required credential is unavailable"))
    } else {
        Ok(value)
    }
}

/// Performs the safe config error operation.
fn safe_config_error(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

impl Settings {
    /// Loads data from persistent storage.
    pub fn load(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        let config: Config = read_json(&root.join("config.json"))?;
        Ok(Self {
            // Sessions use their own agent copy, so a broken default agent only
            // affects `create` (and is reported by `orchid agent`).
            agent: read_json(&root.join("agents/default.json")).unwrap_or_default(),
            root,
            log_level: config.log_level.unwrap_or_else(|| "info".into()),
            hooks: config.hooks,
            session: None,
        })
    }
}

/// Reads a JSON file; errors name the file.
pub fn read_json<T: for<'a> Deserialize<'a>>(path: &Path) -> io::Result<T> {
    let context = |error: &dyn std::fmt::Display| format!("{}: {error}", path.display());
    let bytes = fs::read(path).map_err(|error| io::Error::new(error.kind(), context(&error)))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, context(&error)))
}

pub fn init(root: &Path) -> io::Result<()> {
    for name in ["agents", "connections", "auth", "prompts"] {
        fs::create_dir_all(root.join(name))?;
    }
    write_default(
        &root.join("config.json"),
        br#"{"log_level":"debug"}
"#,
    )?;
    write_default(
        &root.join("prompts/default.txt"),
        b"You are a helpful assistant.",
    )?;
    write_default(
        &root.join("agents/default.json"),
        br#"{"prompt":"default","max_tokens":120000}
"#,
    )
}

/// Performs the write default operation.
fn write_default(path: &Path, content: &[u8]) -> io::Result<()> {
    if !path.exists() {
        fs::write(path, content)?;
    }
    Ok(())
}

pub fn auth(settings: &Settings, args: &[String]) -> io::Result<String> {
    auth_command::run(settings, args)
}
