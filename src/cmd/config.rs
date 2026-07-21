use serde_json::json;
use std::path::Path;

pub fn config_validate(config_dir: &Path) -> Result<serde_json::Value, String> {
    crate::ConfigDir::new(config_dir)
        .validate()
        .map_err(|e| e.to_string())?;
    Ok(json!({"status": "ok", "config": config_dir.display().to_string()}))
}

pub fn config_list(config_dir: &Path) -> Result<serde_json::Value, String> {
    Ok(json!({
        "connections": crate::cmd::list::list(config_dir, Some("connections"))?,
        "policies": crate::cmd::list::list(config_dir, Some("policies"))?,
        "prompts": crate::cmd::list::list(config_dir, Some("prompts"))?,
        "auth": crate::cmd::auth::auth_list(config_dir)?,
    }))
}

pub fn config_show(config_dir: &Path, resource: &str) -> Result<serde_json::Value, String> {
    let root = crate::ConfigDir::new(config_dir);
    match resource {
        "root" | "config" => serde_json::from_str(
            &std::fs::read_to_string(root.root_path()).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string()),
        name if name.starts_with("connection/") => {
            let value = root
                .load_connection(&name[11..])
                .map_err(|e| e.to_string())?;
            serde_json::to_value(value).map_err(|e| e.to_string())
        }
        name if name.starts_with("policy/") => {
            let value = root.load_policy(&name[7..]).map_err(|e| e.to_string())?;
            serde_json::to_value(value).map_err(|e| e.to_string())
        }
        name if name.starts_with("prompt/") => {
            let value = root.load_prompt(&name[7..]).map_err(|e| e.to_string())?;
            Ok(json!({"name": &name[7..], "content": value}))
        }
        name if name.starts_with("auth/") => {
            let value = root.load_auth(&name[5..]).map_err(|e| e.to_string())?;
            Ok(json!({"name": &name[5..], "type": value.kind, "value": value.value}))
        }
        _ => Err(
            "resource must be root, connection/<name>, policy/<name>, prompt/<name>, or auth/<name>".to_string(),
        ),
    }
}
