//! Provides the defs functionality.
use serde_json::{json, Value};

/// Builds definitions for enabled tools.
pub fn tools(enabled: &[String]) -> Vec<Value> {
    enabled.iter().filter_map(|name| {
        let (description, properties) = match name.as_str() {
            "bash" => ("Run a shell command.", json!({"cmd":{"type":"string"}})),
            "fs_read" => ("Read one or more files.", json!({"paths":{"type":"array","items":{"type":"string"}}})),
            "fs_edit" => (
                "Apply edits to one file in order. Each old_string must match exactly once unless replace_all is true; use a distinctive surrounding snippet for reliable edits.",
                json!({
                    "path": {"type":"string", "description":"File path relative to the session working directory."},
                    "edits": {
                        "type":"array",
                        "minItems":1,
                        "description":"Edits are applied sequentially, so later old_string values see earlier changes.",
                        "items": {
                            "type":"object",
                            "additionalProperties":false,
                            "properties": {
                                "old_string": {"type":"string", "description":"Exact text to replace; may be empty only to insert at the start."},
                                "new_string": {"type":"string", "description":"Replacement text."},
                                "replace_all": {"type":"boolean", "default":false, "description":"Replace every occurrence instead of requiring exactly one match."}
                            },
                            "required":["old_string","new_string"]
                        }
                    }
                })
            ),
            _ => return None,
        };
        let required = properties.as_object()?.keys().cloned().collect::<Vec<_>>();
        Some(json!({"type":"function","function":{"name":name,"description":description,"parameters":{"type":"object","properties":properties,"required":required}}}))
    }).collect()
}
