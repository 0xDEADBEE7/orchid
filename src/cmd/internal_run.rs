use crate::{create_provider, load_config, r#loop};

pub fn internal_run(convo_id: &str, profile: &Option<String>) -> Result<(), String> {
    let config = load_config()?;

    let profile_name = profile.clone().unwrap_or(
        config
            .current_profile
            .ok_or_else(|| "no profile configured".to_string())?,
    );

    let profiles = config.profiles;
    let prof = profiles
        .get(&profile_name)
        .ok_or_else(|| format!("profile '{}' not found", profile_name))?;

    let provider = create_provider(prof).map_err(|e| format!("provider error: {}", e))?;

    r#loop::run(convo_id, provider.as_ref())?;

    Ok(())
}


