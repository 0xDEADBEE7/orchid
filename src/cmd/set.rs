use crate::session::{resolve, SessionStore, SessionUpdate};
use std::path::Path;

pub fn set(
    id: String,
    label: Option<String>,
    working_dir: Option<String>,
    restrictions: Option<Vec<String>>,
    config_dir: &Path,
) -> Result<serde_json::Value, String> {
    let store = SessionStore::with_config_dir(config_dir)?;
    let base_path = config_dir.join("sessions");
    let resolved_id = resolve::resolve(&id, &base_path)?.id;

    let mut updates = SessionUpdate::default();

    if let Some(l) = label {
        updates.label = Some(Some(l));
    }
    if let Some(wd) = working_dir {
        updates.working_dir = Some(Some(wd));
    }
    if let Some(values) = restrictions {
        updates.restrictions = Some(Some(values));
    }

    let updated = store.update(&resolved_id, updates)?;
    serde_json::to_value(&updated).map_err(|e| e.to_string())
}
