use globset::GlobSet;
use serde_json::Value;
use std::collections::HashMap;

pub mod bash;
pub mod fs_edit;
pub mod fs_read;
pub mod scope;

pub trait Tool: Send + Sync {
    fn execute(&self, args: Value, working_dir: &str) -> Result<String, String>;
}

pub fn tool_definitions() -> Vec<Value> {
    vec![
        serde_json::json!({"name":"bash","description":"Run a shell command.","input_schema":{"type":"object","properties":{"cmd":{"type":"string"}},"required":["cmd"]}}),
        serde_json::json!({"name":"fs_read","description":"Read files.","input_schema":{"type":"object","properties":{"paths":{"type":"array","items":{"type":"string"}}},"required":["paths"]}}),
        serde_json::json!({"name":"fs_edit","description":"Edit a file.","input_schema":{"type":"object","properties":{"path":{"type":"string"},"edits":{"type":"array"}},"required":["path","edits"]}}),
    ]
}

pub fn execute_tool(
    name: &str,
    input: Value,
    working_dir: &str,
    allow_scope_escape: bool,
    env_vars: &HashMap<String, String>,
    global_scope_set: &GlobSet,
    session_scope_set: &GlobSet,
) -> Result<Value, String> {
    execute_tool_with_permissions(
        name,
        input,
        working_dir,
        allow_scope_escape,
        env_vars,
        global_scope_set,
        session_scope_set,
        &[],
        &[],
    )
}

pub fn execute_tool_with_permissions(
    name: &str,
    input: Value,
    working_dir: &str,
    allow_scope_escape: bool,
    env_vars: &HashMap<String, String>,
    global_scope_set: &GlobSet,
    session_scope_set: &GlobSet,
    allowed_tools: &[String],
    allowed_paths: &[String],
) -> Result<Value, String> {
    if !allowed_tools.is_empty() && !allowed_tools.iter().any(|tool| tool == name || tool == "*") {
        return Err(format!("tool denied by policy: {}", name));
    }
    match name {
        "bash" => bash::execute(
            input,
            working_dir,
            allow_scope_escape,
            env_vars,
            global_scope_set,
            session_scope_set,
            allowed_paths,
        )
        .map(Value::String),
        "fs_read" => fs_read::execute(
            input,
            working_dir,
            allow_scope_escape,
            global_scope_set,
            session_scope_set,
            allowed_paths,
        ),
        "fs_edit" => fs_edit::execute(
            input,
            working_dir,
            allow_scope_escape,
            global_scope_set,
            session_scope_set,
            allowed_paths,
        )
        .map(Value::String),
        _ => Err(format!("unknown tool: {}", name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn rejects_singular_path_input() {
        let paths = fs_read::extract_paths(&serde_json::json!({"path": "file.txt"}));
        assert!(paths.is_empty());
    }

    #[test]
    fn accepts_paths_array_only() {
        let paths = fs_read::extract_paths(&serde_json::json!({"paths": ["a", "b"]}));
        assert_eq!(paths, vec!["a", "b"]);
    }

    #[test]
    fn denies_tools_not_in_policy() {
        let error = execute_tool_with_permissions(
            "bash",
            serde_json::json!({"cmd":"printf denied"}),
            ".",
            false,
            &HashMap::new(),
            &GlobSet::empty(),
            &GlobSet::empty(),
            &["fs_read".to_string()],
            &[],
        )
        .unwrap_err();
        assert_eq!(error, "tool denied by policy: bash");
    }

    #[test]
    fn injects_only_explicit_tool_environment() {
        let result = execute_tool_with_permissions(
            "bash",
            serde_json::json!({"cmd":"printf %s \\\"$ORCHID_TOOL_TEST\\\""}),
            ".",
            false,
            &HashMap::from([("ORCHID_TOOL_TEST".to_string(), "runtime-secret".to_string())]),
            &GlobSet::empty(),
            &GlobSet::empty(),
            &["bash".to_string()],
            &[],
        )
        .unwrap();
        assert_eq!(result, serde_json::json!("\"runtime-secret\""));
    }

    #[test]
    fn denies_paths_outside_policy() {
        let temp = TempDir::new().unwrap();
        let allowed = temp.path().join("allowed.txt");
        let denied = temp.path().join("denied.txt");
        std::fs::write(&allowed, "allowed").unwrap();
        std::fs::write(&denied, "denied").unwrap();
        let result = execute_tool_with_permissions(
            "fs_read",
            serde_json::json!({"paths":[denied.to_string_lossy()]}),
            ".",
            false,
            &HashMap::new(),
            &GlobSet::empty(),
            &GlobSet::empty(),
            &["fs_read".to_string()],
            &[allowed.to_string_lossy().to_string()],
        );
        assert!(result.unwrap_err().contains("out of scope"));
    }
}
