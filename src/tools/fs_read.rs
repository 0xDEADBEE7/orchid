use crate::tools::scope::is_allowed;
use globset::GlobSet;
use serde_json::Value;
use std::fs;

/// Extract paths from an fs_read tool call input.
pub fn extract_paths(input: &Value) -> Vec<String> {
    input
        .get("paths")
        .and_then(|v| v.as_array())
        .map(|paths| {
            paths
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

/// Returns a JSON object: `{"<path>": "<content>", ...}`.
/// Errors in individual files are represented as `{"error": "<msg>"}` values.
/// A single-path read that fails propagates the error directly.
pub fn execute(
    input: Value,
    working_dir: &str,
    global_scope_set: &GlobSet,
    session_scope_set: &GlobSet,
    allowed_paths: &[String],
) -> Result<Value, String> {
    let paths = extract_paths(&input);

    if paths.is_empty() {
        return Err("invalid fs_read input: expected non-empty 'paths' array".to_string());
    }

    if paths.len() == 1 {
        let content = read_one(
            &paths[0],
            working_dir,
            global_scope_set,
            session_scope_set,
            allowed_paths,
        )?;
        Ok(serde_json::json!({ &paths[0]: content }))
    } else {
        let mut map = serde_json::Map::new();
        for path in &paths {
            match read_one(
                path,
                working_dir,
                global_scope_set,
                session_scope_set,
                allowed_paths,
            ) {
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
    global_scope_set: &GlobSet,
    session_scope_set: &GlobSet,
    allowed_paths: &[String],
) -> Result<String, String> {
    if !is_allowed(path, working_dir, global_scope_set, session_scope_set)
        || !crate::tools::scope::is_allowed_by_policy(path, working_dir, allowed_paths)
    {
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
