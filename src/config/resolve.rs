use crate::config::Connection;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct EffectiveSessionConfig {
    pub policy_name: String,
    pub policy_hash: String,
    pub connection_candidates: Vec<Connection>,
    pub prompt: String,
    pub working_dir: PathBuf,
    pub permissions: crate::config::Permissions,
    pub limits: crate::config::PolicyLimits,
    pub env_vars: HashMap<String, String>,
}

pub fn resolve(
    config_dir: &crate::config::ConfigDir,
    explicit_policy: Option<&str>,
    working_dir: Option<&str>,
) -> Result<EffectiveSessionConfig, String> {
    let root = config_dir.load_root().map_err(|e| e.to_string())?;
    let policy_name = explicit_policy
        .unwrap_or(&root.policy)
        .to_string();

    let policy = config_dir.load_policy(&policy_name).map_err(|e| e.to_string())?;

    let policy_path = config_dir.policies_path().join(format!("{}.json", &policy_name));
    let policy_hash = compute_policy_hash(&policy_path);

    let mut connection_candidates = Vec::new();
    for conn_name in &policy.connections {
        let conn = config_dir.load_connection(conn_name).map_err(|e| e.to_string())?;
        connection_candidates.push(conn);
    }

    let prompt = if let Some(prompt_name) = &policy.prompt {
        config_dir.load_prompt(prompt_name).map_err(|e| e.to_string())?
    } else {
        String::new()
    };

    let working_dir = working_dir
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| "no working directory configured".to_string())?;

    let mut env_vars = HashMap::new();
    for conn in &connection_candidates {
        if let Some(ref api_key) = conn.api_key {
            if let Some(var_name) = api_key.strip_prefix("env.") {
                let value = env::var(var_name).unwrap_or_default();
                env_vars.insert(var_name.to_string(), value);
            }
        }
        for (key, value) in &conn.headers {
            let resolved = resolve_env_inline(value);
            if resolved != *value {
                env_vars.insert(key.clone(), resolved);
            }
        }
    }

    Ok(EffectiveSessionConfig {
        policy_name,
        policy_hash,
        connection_candidates,
        prompt,
        working_dir,
        permissions: policy.permissions,
        limits: policy.limits,
        env_vars,
    })
}

fn compute_policy_hash(path: &Path) -> String {
    let contents = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return "read_error".to_string(),
    };
    let mut hasher = DefaultHasher::new();
    contents.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

pub fn resolve_env_inline(s: &str) -> String {
    let mut result = s.to_string();
    while let Some(start) = result.find("env.") {
        let after = &result[start + 4..];
        let end = after
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(after.len());
        let var_name = &after[..end];
        let value = env::var(var_name).unwrap_or_default();
        result = format!("{}{}{}", &result[..start], value, &after[end..]);
    }
    result
}
