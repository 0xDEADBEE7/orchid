use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

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
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Policy {
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub log_level: Option<String>,
    #[serde(default)]
    pub connections: Vec<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub hooks: Vec<String>,
    #[serde(default)]
    pub permissions: Permissions,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Permissions {
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub paths: Vec<String>,
}

impl Policy {
    pub fn tools(&self) -> &[String] {
        if self.permissions.tools.is_empty() {
            &self.tools
        } else {
            &self.permissions.tools
        }
    }
    pub fn paths(&self) -> &[String] {
        if self.permissions.paths.is_empty() {
            &self.paths
        } else {
            &self.permissions.paths
        }
    }
    pub fn max_tokens(&self) -> u32 {
        self.max_tokens.unwrap_or(4096)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    pub interface: String,
    pub base_url: String,
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
    pub policy_name: String,
    pub policy: Policy,
    pub prompt_name: String,
    pub log_level: String,
}

impl Settings {
    pub fn prompt(&self) -> io::Result<String> {
        let name = &self.prompt_name;
        let prompts = self.root.join("prompts");
        let markdown = prompts.join(format!("{name}.md"));
        let text = prompts.join(format!("{name}.txt"));
        if markdown.exists() {
            fs::read_to_string(markdown)
        } else {
            fs::read_to_string(text)
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
    pub fn connection(&self, name: &str) -> io::Result<Connection> {
        read_json(&self.root.join("connections").join(format!("{name}.json")))
    }

    pub fn resolve_connection(&self, name: &str) -> io::Result<ResolvedConnection> {
        let connection = self.connection(name)?;
        if connection.interface.is_empty()
            || connection.base_url.is_empty()
            || connection.model.is_empty()
        {
            return Err(safe_config_error(
                "connection requires interface, base_url, and model",
            ));
        }
        if !matches!(
            connection.interface.as_str(),
            "local" | "openai" | "anthropic" | "codex"
        ) {
            return Err(safe_config_error("unknown connection interface"));
        }
        let credential = if connection.interface == "local" {
            None
        } else if let Some(auth) = connection.auth.as_deref() {
            self.resolve_auth(auth)?
        } else {
            connection
                .api_key
                .as_deref()
                .map(resolve_secret)
                .transpose()?
                .map(Credential::ApiKey)
        };
        let headers = connection
            .headers
            .iter()
            .map(|(name, value)| resolve_inline(value).map(|value| (name.clone(), value)))
            .collect::<io::Result<HashMap<_, _>>>()?;
        let params = connection.params.clone();
        Ok(ResolvedConnection {
            connection,
            credential,
            params,
            headers,
        })
    }

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

fn resolve_inline(reference: &str) -> io::Result<String> {
    if reference.starts_with("env.") {
        return resolve_secret(reference);
    }
    Ok(reference.to_owned())
}

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

fn safe_config_error(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

impl Settings {
    pub fn load(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        let config: Config = read_json(&root.join("config.json"))?;
        let policy_name = "default".to_owned();
        let policy: Policy = read_json(&root.join("policies").join(format!("{policy_name}.json")))?;
        Ok(Self {
            root,
            policy_name,
            policy,
            prompt_name: "default".into(),
            log_level: config.log_level.unwrap_or_else(|| "info".into()),
        })
    }
}

pub fn read_json<T: for<'a> Deserialize<'a>>(path: &Path) -> io::Result<T> {
    let bytes = fs::read(path)?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

pub fn init(root: &Path) -> io::Result<()> {
    for name in ["agents", "policies", "connections", "auth", "prompts"] {
        fs::create_dir_all(root.join(name))?;
    }
    write_default(
        &root.join("config.json"),
        br#"{"log_level":"debug"}
"#,
    )?;
    write_default(
        &root.join("policies/default.json"),
        br#"{"max_tokens":4096,"tools":[]}
"#,
    )?;
    write_default(
        &root.join("prompts/default.txt"),
        b"You are a helpful assistant.",
    )?;
    write_default(
        &root.join("agents/default.json"),
        br#"{"policy":"default","prompt":"default"}
"#,
    )
}

fn write_default(path: &Path, content: &[u8]) -> io::Result<()> {
    if !path.exists() {
        fs::write(path, content)?;
    }
    Ok(())
}

pub fn empty_env() -> HashMap<String, String> {
    HashMap::new()
}

pub fn command(settings: &Settings, args: &[String]) -> io::Result<String> {
    match args.first().map(String::as_str) {
        None | Some("show") => serde_json::to_string(&settings.policy).map_err(io::Error::other),
        Some("validate") => Ok(serde_json::json!({"valid":true}).to_string()),
        Some("list") => Ok(serde_json::json!({"policies":[settings.policy_name]}).to_string()),
        Some(name) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown config command: {name}"),
        )),
    }
}

pub fn auth(settings: &Settings, args: &[String]) -> io::Result<String> {
    match args.first().map(String::as_str) {
        None | Some("list") => auth_list(settings),
        Some("validate") => auth_validate(settings, args.get(1)),
        Some("login") => auth_login(settings, args.get(1)),
        Some(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unknown auth command",
        )),
    }
}

fn auth_list(settings: &Settings) -> io::Result<String> {
    let names = fs::read_dir(settings.root.join("auth"))?
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().into_string().ok())
        .collect::<Vec<_>>();
    serde_json::to_string(&serde_json::json!({"auth":names})).map_err(io::Error::other)
}

fn auth_validate(settings: &Settings, name: Option<&String>) -> io::Result<String> {
    let name = name.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "auth validate requires a name")
    })?;
    if settings
        .root
        .join("auth/tokens")
        .join(format!("{name}.json"))
        .exists()
    {
        let _ = crate::client::openai_codex::auth::access_token(&settings.root, name).map_err(
            |_| io::Error::new(io::ErrorKind::InvalidData, "Codex credential unavailable"),
        )?;
        return Ok(serde_json::json!({"valid":true,"type":"openai_codex_oauth"}).to_string());
    }
    let _: Value = read_json(&settings.root.join("auth").join(format!("{name}.json")))?;
    Ok(serde_json::json!({"valid":true}).to_string())
}

fn auth_login(settings: &Settings, name: Option<&String>) -> io::Result<String> {
    let name = name
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "auth login requires a name"))?;
    if name == "codex"
        || settings
            .root
            .join("auth/tokens")
            .join(format!("{name}.json"))
            .exists()
    {
        return crate::client::openai_codex::auth::login(&settings.root, name)
            .map_err(io::Error::other)
            .map(|value| value.to_string());
    }
    let key = std::env::var("ORCHID_API_KEY")
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "ORCHID_API_KEY is required"))?;
    let body = serde_json::to_vec(&serde_json::json!({"type":"api_key","value":key}))
        .map_err(io::Error::other)?;
    fs::create_dir_all(settings.root.join("auth"))?;
    fs::write(
        settings.root.join("auth").join(format!("{name}.json")),
        body,
    )?;
    Ok(serde_json::json!({"name":name}).to_string())
}
