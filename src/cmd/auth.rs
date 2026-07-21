use serde_json::json;
use std::path::Path;

pub fn auth_list(config_dir: &Path) -> Result<serde_json::Value, String> {
    let dir = crate::ConfigDir::new(config_dir);
    let entries = std::fs::read_dir(dir.auth_path()).map_err(|e| e.to_string())?;
    let mut profiles = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|n| n.to_str())
            .ok_or_else(|| "invalid auth profile filename".to_string())?;
        let profile = dir.load_auth(name).map_err(|e| e.to_string())?;
        profiles.push(json!({"name": name, "type": profile.kind, "value": profile.value}));
    }
    profiles.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    Ok(json!({"profiles": profiles}))
}

pub fn auth_validate(config_dir: &Path, name: &str) -> Result<serde_json::Value, String> {
    let dir = crate::ConfigDir::new(config_dir);
    let profile = dir.load_auth(name).map_err(|e| e.to_string())?;
    let value = crate::client::resolve::resolve_auth(&profile).map_err(|e| e.to_string())?;
    Ok(
        json!({"status": "ok", "name": name, "type": profile.kind, "credential_present": !value.is_empty()}),
    )
}
