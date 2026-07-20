use crate::session::resolve;
use serde_json::json;
use std::fs;
use std::path::Path;

pub fn delete(id: String, config_dir: &Path) -> Result<serde_json::Value, String> {
    let base_path = config_dir.join("sessions");
    let resolved_id = resolve::resolve(&id, &base_path)?.id;
    let convo_path = base_path.join(&resolved_id);

    if !convo_path.exists() {
        return Err(format!("conversation '{}' not found", id));
    }

    let archive_base = base_path.join(".archive");

    fs::create_dir_all(&archive_base)
        .map_err(|e| format!("failed to create archive dir: {}", e))?;

    let archive_path = archive_base.join(&resolved_id);

    fs::rename(&convo_path, &archive_path)
        .map_err(|e| format!("failed to move conversation to archive: {}", e))?;

    Ok(json!({
        "id": resolved_id,
        "status": "archived",
        "archived_at": chrono::Utc::now().to_rfc3339()
    }))
}
