use crate::{config::resolve::{resolve as resolve_effective_config, create_provider_from_connection}, config::ConfigDir, r#loop};

pub fn internal_run(convo_id: &str, config_dir: &std::path::Path) -> Result<(), String> {
    let config_dir_ref = ConfigDir::new(config_dir);
    let effective = resolve_effective_config(&config_dir_ref, None, None)
        .map_err(|e| format!("failed to resolve effective config: {}", e))?;

    let connection = effective
        .connection_candidates
        .first()
        .ok_or_else(|| "policy has no connections configured".to_string())?;

    let log_path = config_dir_ref
        .sessions_path()
        .join(convo_id)
        .join("orchid.log");

    let provider =
        create_provider_from_connection(connection, Some(log_path)).map_err(|e| format!("provider error: {}", e))?;

    r#loop::run(convo_id, &effective, config_dir, provider.as_ref())?;

    Ok(())
}
