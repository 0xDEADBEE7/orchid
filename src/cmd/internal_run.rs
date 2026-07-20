use crate::{
    config::resolve::{
        create_provider_from_connections_with_log, resolve as resolve_effective_config,
    },
    config::ConfigDir,
    convo::Store,
    r#loop,
};

pub fn internal_run(convo_id: &str, config_dir: &std::path::Path) -> Result<(), String> {
    let config_dir_ref = ConfigDir::new(config_dir);
    let root = config_dir_ref
        .load_root()
        .map_err(|e| format!("failed to resolve effective config: {}", e))?;
    let store = Store::with_config_dir(config_dir)?;
    let meta = store.get(convo_id)?;
    let policy = meta.policy.as_deref().unwrap_or(&root.policy);
    let effective =
        resolve_effective_config(&config_dir_ref, Some(policy), meta.working_dir.as_deref())
            .map_err(|e| format!("failed to resolve effective config: {}", e))?;

    let log_path = config_dir_ref
        .sessions_path()
        .join(convo_id)
        .join("orchid.log");
    let provider =
        create_provider_from_connections_with_log(&effective.connection_candidates, Some(log_path))
            .map_err(|e| format!("provider error: {}", e))?;
    r#loop::run(convo_id, &effective, config_dir, provider.as_ref())
}
