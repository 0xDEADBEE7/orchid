use crate::tools::scope::is_allowed;
use globset::GlobSet;
use serde_json::Value;
use std::fs;

/// Extract paths from an fs_read tool call input.
/// Supports both `{"paths": [...]}` (batch) and legacy `{"path": "..."}`.
pub fn extract_paths(input: &Value) -> Vec<String> {
    if let Some(paths) = input.get("paths").and_then(|v| v.as_array()) {
        paths
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect()
    } else if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
        vec![path.to_string()]
    } else {
        vec![]
    }
}

/// Returns a JSON object: `{"<path>": "<content>", ...}`.
/// Errors in individual files are represented as `{"error": "<msg>"}` values.
/// A single-path read that fails propagates the error directly.
pub fn execute(
    input: Value,
    working_dir: &str,
    allow_scope_escape: bool,
    global_scope_set: &GlobSet,
    convo_scope_set: &GlobSet,
) -> Result<Value, String> {
    let paths = extract_paths(&input);

    if paths.is_empty() {
        return Err("invalid fs_read input: expected 'paths' array or 'path' string".to_string());
    }

    if paths.len() == 1 {
        let content = read_one(&paths[0], working_dir, allow_scope_escape, global_scope_set, convo_scope_set)?;
        Ok(serde_json::json!({ &paths[0]: content }))
    } else {
        let mut map = serde_json::Map::new();
        for path in &paths {
            match read_one(path, working_dir, allow_scope_escape, global_scope_set, convo_scope_set) {
                Ok(content) => {
                    map.insert(path.clone(), Value::String(content));
                }
                Err(e) => {
                    map.insert(path.clone(), serde_json::json!({"error": e}));
                }
            }
        }
        Ok(Value::Object(map))
    }
}

fn read_one(
    path: &str,
    working_dir: &str,
    allow_scope_escape: bool,
    global_scope_set: &GlobSet,
    convo_scope_set: &GlobSet,
) -> Result<String, String> {
    if !allow_scope_escape && !is_allowed(path, working_dir, global_scope_set, convo_scope_set) {
        return Err(format!("path out of scope: {}", path));
    }

    let resolved = crate::tools::scope::expand_path(path, working_dir);
    fs::read_to_string(&resolved).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!("file not found: {}", path)
        } else if e.kind() == std::io::ErrorKind::PermissionDenied {
            format!("permission denied: {}", path)
        } else {
            format!("failed to read file: {}", e)
        }
    })
}


