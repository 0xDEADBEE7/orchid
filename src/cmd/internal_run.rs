use crate::{
    config::resolve::{
        create_provider_from_connection, resolve as resolve_effective_config,
        EffectiveSessionConfig,
    },
    config::ConfigDir,
    convo::Store,
    r#loop,
};
use std::fs;

pub fn internal_run(convo_id: &str, config_dir: &std::path::Path) -> Result<(), String> {
    internal_run_with_snapshot(convo_id, config_dir, None)
}

pub fn internal_run_with_snapshot(
    convo_id: &str,
    config_dir: &std::path::Path,
    effective_config: Option<&str>,
) -> Result<(), String> {
    let config_dir_ref = ConfigDir::new(config_dir);
    let effective = if let Some(path) = effective_config {
        let contents = fs::read_to_string(path)
            .map_err(|e| format!("failed to read effective config {}: {}", path, e))?;
        serde_json::from_str::<EffectiveSessionConfig>(&contents)
            .map_err(|e| format!("invalid effective config {}: {}", path, e))?
    } else {
        let store = Store::with_config_dir(config_dir)?;
        if let Some(snapshot) = store.read_snapshot(convo_id)? {
            snapshot
        } else {
            let root = config_dir_ref
                .load_root()
                .map_err(|e| format!("failed to resolve effective config: {}", e))?;
            let meta = store.get(convo_id)?;
            let policy = meta.policy.as_deref().unwrap_or(&root.policy);
            resolve_effective_config(&config_dir_ref, Some(policy), meta.working_dir.as_deref())
                .map_err(|e| format!("failed to resolve effective config: {}", e))?
        }
    };

    let connection = effective
        .connection_candidates
        .first()
        .ok_or_else(|| "policy has no connections configured".to_string())?;

    let log_path = config_dir_ref
        .sessions_path()
        .join(convo_id)
        .join("orchid.log");

    let provider = create_provider_from_connection(connection, Some(log_path))
        .map_err(|e| format!("provider error: {}", e))?;

    r#loop::run(convo_id, &effective, config_dir, provider.as_ref())?;

    Ok(())
}
