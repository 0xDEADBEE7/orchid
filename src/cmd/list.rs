use crate::SessionStore;
use serde_json::json;
use std::path::Path;

pub fn list(config_dir: &Path, resource: Option<&str>) -> Result<serde_json::Value, String> {
    match resource.unwrap_or("sessions") {
        "sessions" => {
            let store = SessionStore::with_config_dir(config_dir)?;
            Ok(json!(store.list()?))
        }
        kind @ ("connections" | "policies" | "prompts" | "auth") => {
            list_resources(config_dir, kind)
        }
        _ => Err("unknown resource".to_string()),
    }
}

fn list_resources(config_dir: &Path, kind: &str) -> Result<serde_json::Value, String> {
    let dir = config_dir.join(kind);
    let suffix = if kind == "prompts" { ".md" } else { ".json" };
    let mut names = Vec::new();
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().and_then(|e| e.to_str()) == Some(&suffix[1..]) {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    names.push(stem.to_string());
                }
            }
        }
    }
    names.sort();
    Ok(json!(names))
}
