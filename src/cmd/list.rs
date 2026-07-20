use crate::load_config;
use crate::Store;
use serde_json::json;
use std::path::Path;

pub fn list(config_dir: &Path) -> Result<serde_json::Value, String> {
    let store = Store::with_config_dir(config_dir)?;
    let convos = store.list()?;

    Ok(json!(convos))
}

pub fn list_profiles() -> Result<serde_json::Value, String> {
    let config = load_config()?;
    Ok(json!(config.profiles))
}

pub fn list_personas() -> Result<serde_json::Value, String> {
    let config = load_config()?;
    let personas = config.extra.get("personas").cloned().unwrap_or(json!({}));
    Ok(personas)
}
