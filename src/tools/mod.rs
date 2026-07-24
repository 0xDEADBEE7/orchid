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

pub struct ToolContext<'a> {
    pub working_dir: &'a str,
    pub env_vars: &'a HashMap<String, String>,
    pub global_scope_set: &'a GlobSet,
    pub session_scope_set: &'a GlobSet,
    pub allowed_tools: &'a [String],
    pub allowed_paths: &'a [String],
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
    env_vars: &HashMap<String, String>,
    global_scope_set: &GlobSet,
    session_scope_set: &GlobSet,
) -> Result<Value, String> {
    execute_tool_with_permissions(
        name,
        input,
        &ToolContext {
            working_dir,
            env_vars,
            global_scope_set,
            session_scope_set,
            allowed_tools: &[],
            allowed_paths: &[],
        },
    )
}

pub fn execute_tool_with_permissions(
    name: &str,
    input: Value,
    context: &ToolContext<'_>,
) -> Result<Value, String> {
    if !context.allowed_tools.is_empty()
        && !context
            .allowed_tools
            .iter()
            .any(|tool| tool == name || tool == "*")
    {
        return Err(format!("tool denied by policy: {}", name));
    }
    match name {
        "bash" => bash::execute(
            input,
            context.working_dir,
            context.env_vars,
            context.global_scope_set,
            context.session_scope_set,
            context.allowed_paths,
        )
        .map(Value::String),
        "fs_read" => fs_read::execute(
            input,
            context.working_dir,
            context.global_scope_set,
            context.session_scope_set,
            context.allowed_paths,
        ),
        "fs_edit" => fs_edit::execute(
            input,
            context.working_dir,
            context.global_scope_set,
            context.session_scope_set,
            context.allowed_paths,
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
            &ToolContext {
                working_dir: ".",
                env_vars: &HashMap::new(),
                global_scope_set: &GlobSet::empty(),
                session_scope_set: &GlobSet::empty(),
                allowed_tools: &["fs_read".to_string()],
                allowed_paths: &[],
            },
        )
        .unwrap_err();
        assert_eq!(error, "tool denied by policy: bash");
    }

    #[test]
    fn injects_only_explicit_tool_environment() {
        let result = execute_tool_with_permissions(
            "bash",
            serde_json::json!({"cmd":"printf %s \\\"$ORCHID_TOOL_TEST\\\""}),
            &ToolContext {
                working_dir: ".",
                env_vars: &HashMap::from([("ORCHID_TOOL_TEST".to_string(), "runtime-secret".to_string())]),
                global_scope_set: &GlobSet::empty(),
                session_scope_set: &GlobSet::empty(),
                allowed_tools: &["bash".to_string()],
                allowed_paths: &[],
            },
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
            &ToolContext {
                working_dir: ".",
                env_vars: &HashMap::new(),
                global_scope_set: &GlobSet::empty(),
                session_scope_set: &GlobSet::empty(),
                allowed_tools: &["fs_read".to_string()],
                allowed_paths: &[allowed.to_string_lossy().to_string()],
            },
        );
        assert!(result.unwrap_err().contains("out of scope"));
    }
}
