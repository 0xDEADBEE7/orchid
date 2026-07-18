use crate::load_config;
use crate::Store;
use serde_json::json;

pub fn list() -> Result<serde_json::Value, String> {
    let store = Store::new()?;
    let convos = store.list()?;

    let json_array = json!(convos);
    Ok(json_array)
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


