use crate::config::resolve::resolve as resolve_effective_config;
use crate::config::ConfigDir;
use crate::session::SessionStore;

pub fn create(
    label: Option<String>,
    working_dir: Option<String>,
    scope_exceptions: Option<Vec<String>>,
    policy: Option<String>,
    config_dir: &std::path::Path,
) -> Result<serde_json::Value, String> {
    let wd = resolve_working_dir(working_dir)?;
    let effective =
        resolve_effective_config(&ConfigDir::new(config_dir), policy.as_deref(), Some(&wd))
            .map_err(|e| format!("failed to resolve effective config: {}", e))?;
    let store = SessionStore::with_config_dir(config_dir)?;
    let meta = store.create(label, Some(wd), scope_exceptions)?;
    let meta = store.update(
        &meta.id,
        crate::SessionUpdate {
            policy: Some(Some(effective.policy_name)),
            policy_hash: Some(Some(effective.policy_hash)),
            ..Default::default()
        },
    )?;
    serde_json::to_value(&meta).map_err(|e| e.to_string())
}

pub fn resolve_working_dir(working_dir: Option<String>) -> Result<String, String> {
    match working_dir {
        Some(wd) => Ok(wd),
        None => std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .map_err(|e| format!("failed to get current directory: {}", e)),
    }
}
