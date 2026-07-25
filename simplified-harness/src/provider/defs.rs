use serde_json::{json, Value};

pub fn tools(enabled: &[String]) -> Vec<Value> {
    enabled.iter().filter_map(|name| {
        let (description, properties) = match name.as_str() {
            "bash" => ("Run a shell command.", json!({"cmd":{"type":"string"}})),
            "fs_read" => ("Read one or more files.", json!({"paths":{"type":"array","items":{"type":"string"}}})),
            "fs_edit" => ("Edit a file.", json!({"path":{"type":"string"},"edits":{"type":"array"}})),
            _ => return None,
        };
        let required = properties.as_object()?.keys().cloned().collect::<Vec<_>>();
        Some(json!({"type":"function","function":{"name":name,"description":description,"parameters":{"type":"object","properties":properties,"required":required}}}))
    }).collect()
}
