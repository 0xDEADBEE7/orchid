use crate::config::Connection;
use crate::provider::{Provider, ProviderError};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectiveSessionConfig {
    pub policy_name: String,
    pub policy_hash: String,
    pub connection_candidates: Vec<super::Connection>,
    pub prompt: String,
    pub working_dir: PathBuf,
    pub permissions: super::Permissions,
    pub limits: super::PolicyLimits,
}

pub fn resolve(
    config_dir: &crate::config::ConfigDir,
    explicit_policy: Option<&str>,
    working_dir: Option<&str>,
) -> Result<EffectiveSessionConfig, String> {
    let root = config_dir.load_root().map_err(|e| e.to_string())?;
    let policy_name = explicit_policy.unwrap_or(&root.policy).to_string();

    let policy = config_dir
        .load_policy(&policy_name)
        .map_err(|e| e.to_string())?;

    let policy_path = config_dir
        .policies_path()
        .join(format!("{}.json", &policy_name));
    let policy_hash = compute_policy_hash(&policy_path);

    let mut connection_candidates = Vec::new();
    for conn_name in &policy.connections {
        let conn = config_dir
            .load_connection(conn_name)
            .map_err(|e| e.to_string())?;
        connection_candidates.push(conn);
    }

    let prompt = if let Some(prompt_name) = &policy.prompt {
        config_dir
            .load_prompt(prompt_name)
            .map_err(|e| e.to_string())?
    } else {
        String::new()
    };

    let working_dir = working_dir
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| "no working directory configured".to_string())?;

    Ok(EffectiveSessionConfig {
        policy_name,
        policy_hash,
        connection_candidates,
        prompt,
        working_dir,
        permissions: policy.permissions,
        limits: policy.limits,
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

pub fn create_provider_from_connection(
    conn: &Connection,
    log_path: Option<PathBuf>,
) -> Result<Arc<dyn Provider>, ProviderError> {
    crate::client::create_provider_from_connection_with_log(conn, log_path)
}
pub fn create_provider_from_connections_with_log(
    connections: &[Connection],
    log_path: Option<PathBuf>,
) -> Result<Arc<dyn Provider>, ProviderError> {
    crate::client::create_provider_from_connections_with_log(connections, log_path)
}
