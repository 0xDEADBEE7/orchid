use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub policy: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Policy {
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub max_steps: Option<u32>,
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
    pub fn max_steps(&self) -> u32 {
        self.max_steps.unwrap_or(8)
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
}

impl Settings {
    pub fn connection(&self, name: &str) -> io::Result<Connection> {
        read_json(&self.root.join("connections").join(format!("{name}.json")))
    }
}

impl Settings {
    pub fn load(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        let config: Config = read_json(&root.join("config.json"))?;
        let policy_name = config.policy.unwrap_or_else(|| "default".into());
        let policy: Policy = read_json(&root.join("policies").join(format!("{policy_name}.json")))?;
        Ok(Self {
            root,
            policy_name,
            policy,
        })
    }
}

pub fn read_json<T: for<'a> Deserialize<'a>>(path: &Path) -> io::Result<T> {
    let bytes = fs::read(path)?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

pub fn init(root: &Path) -> io::Result<()> {
    fs::create_dir_all(root.join("policies"))?;
    fs::create_dir_all(root.join("connections"))?;
    fs::create_dir_all(root.join("auth"))?;
    let config = root.join("config.json");
    if !config.exists() {
        fs::write(
            config,
            br#"{"policy":"default"}
"#,
        )?;
    }
    let policy = root.join("policies/default.json");
    if !policy.exists() {
        fs::write(
            policy,
            br#"{"prompt":"You are a helpful assistant.","max_steps":8,"tools":[]}
"#,
        )?;
    }
    Ok(())
}

pub fn empty_env() -> HashMap<String, String> {
    HashMap::new()
}

pub fn command(settings: &Settings, args: &[String]) -> io::Result<String> {
    match args.first().map(String::as_str) {
        None | Some("show") => serde_json::to_string(&settings.policy).map_err(io::Error::other),
        Some("validate") => Ok("valid".into()),
        Some("list") => Ok(format!("policy/{}", settings.policy_name)),
        Some(name) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown config command: {name}"),
        )),
    }
}

pub fn auth(settings: &Settings, args: &[String]) -> io::Result<String> {
    match args.first().map(String::as_str) {
        None | Some("list") => {
            let names = fs::read_dir(settings.root.join("auth"))?
                .filter_map(Result::ok)
                .filter_map(|e| e.file_name().into_string().ok())
                .collect::<Vec<_>>();
            serde_json::to_string(&names).map_err(io::Error::other)
        }
        Some("validate") => {
            let name = args.get(1).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "auth validate requires a name")
            })?;
            let _: Value = read_json(&settings.root.join("auth").join(name))?;
            Ok("valid".into())
        }
        Some("login") => {
            let name = args.get(1).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "auth login requires a name")
            })?;
            let key = std::env::var("ORCHID_API_KEY").map_err(|_| {
                io::Error::new(io::ErrorKind::NotFound, "ORCHID_API_KEY is required")
            })?;
            fs::write(
                settings.root.join("auth").join(name),
                serde_json::to_vec(&serde_json::json!({"type":"api_key","value":key}))
                    .map_err(io::Error::other)?,
            )?;
            Ok(name.clone())
        }
        Some(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unknown auth command",
        )),
    }
}
