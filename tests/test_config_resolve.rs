mod support;
use orchid::config::{ConfigDir, Connection, Permissions, Policy, PolicyLimits, RootConfig};
use orchid::config::resolve::{resolve as resolve_effective_config, EffectiveSessionConfig};
use std::collections::HashMap;
use support::TestEnv;

fn write_config(dir: &std::path::Path, policy_name: &str) {
    let config = serde_json::json!({ "policy": policy_name });
    std::fs::write(dir.join("config.json"), config.to_string()).unwrap();
}

fn write_connection(dir: &std::path::Path, name: &str, iface: &str, base_url: &str, model: &str) {
    let conn = serde_json::json!({
        "interface": iface,
        "base_url": base_url,
        "model": model,
    });
    std::fs::write(
        dir.join("connections").join(format!("{}.json", name)),
        conn.to_string(),
    )
    .unwrap();
}

fn write_policy(
    dir: &std::path::Path,
    name: &str,
    connections: &[&str],
    prompt: Option<&str>,
) {
    let policy = serde_json::json!({
        "connections": connections,
        "prompt": prompt,
    });
    std::fs::write(
        dir.join("policies").join(format!("{}.json", name)),
        policy.to_string(),
    )
    .unwrap();
}

fn write_prompt(dir: &std::path::Path, name: &str, content: &str) {
    std::fs::write(dir.join("prompts").join(format!("{}.md", name)), content).unwrap();
}

#[test]
fn test_resolve_uses_root_policy() {
    let env = TestEnv::new();
    let dir = env.dir();
    std::fs::create_dir_all(dir.join("connections")).unwrap();
    std::fs::create_dir_all(dir.join("policies")).unwrap();
    std::fs::create_dir_all(dir.join("prompts")).unwrap();

    write_config(&dir, "default");
    write_connection(&dir, "local", "openai", "http://localhost:1234", "local-model");
    write_policy(&dir, "default", &["local"], None);

    let config_dir = ConfigDir::new(&dir);
    let result = resolve_effective_config(&config_dir, None, Some("/tmp"));
    assert!(result.is_ok());
    let cfg = result.unwrap();
    assert_eq!(cfg.policy_name, "default");
    assert_eq!(cfg.connection_candidates.len(), 1);
    assert_eq!(cfg.connection_candidates[0].interface, "openai");
}

#[test]
fn test_resolve_explicit_policy_overrides_root() {
    let env = TestEnv::new();
    let dir = env.dir();
    std::fs::create_dir_all(dir.join("connections")).unwrap();
    std::fs::create_dir_all(dir.join("policies")).unwrap();
    std::fs::create_dir_all(dir.join("prompts")).unwrap();

    write_config(&dir, "default");
    write_connection(&dir, "fast", "openai", "http://localhost:1234", "fast-model");
    write_connection(&dir, "smart", "openai", "http://localhost:1235", "smart-model");
    write_policy(&dir, "default", &["fast"], None);
    write_policy(&dir, "advanced", &["fast", "smart"], None);

    let config_dir = ConfigDir::new(&dir);
    let result = resolve_effective_config(&config_dir, Some("advanced"), Some("/tmp"));
    assert!(result.is_ok());
    let cfg = result.unwrap();
    assert_eq!(cfg.policy_name, "advanced");
    assert_eq!(cfg.connection_candidates.len(), 2);
}

#[test]
fn test_resolve_missing_policy_fails() {
    let env = TestEnv::new();
    let dir = env.dir();
    std::fs::create_dir_all(dir.join("connections")).unwrap();
    std::fs::create_dir_all(dir.join("policies")).unwrap();
    std::fs::create_dir_all(dir.join("prompts")).unwrap();

    write_config(&dir, "nonexistent");
    let config_dir = ConfigDir::new(&dir);
    let result = resolve_effective_config(&config_dir, None, Some("/tmp"));
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("nonexistent"));
}

#[test]
fn test_resolve_missing_connection_fails() {
    let env = TestEnv::new();
    let dir = env.dir();
    std::fs::create_dir_all(dir.join("connections")).unwrap();
    std::fs::create_dir_all(dir.join("policies")).unwrap();
    std::fs::create_dir_all(dir.join("prompts")).unwrap();

    write_config(&dir, "default");
    write_policy(&dir, "default", &["missing-conn"], None);

    let config_dir = ConfigDir::new(&dir);
    let result = resolve_effective_config(&config_dir, None, Some("/tmp"));
    assert!(result.is_err());
}

#[test]
fn test_resolve_with_prompt() {
    let env = TestEnv::new();
    let dir = env.dir();
    std::fs::create_dir_all(dir.join("connections")).unwrap();
    std::fs::create_dir_all(dir.join("policies")).unwrap();
    std::fs::create_dir_all(dir.join("prompts")).unwrap();

    write_config(&dir, "default");
    write_connection(&dir, "local", "openai", "http://localhost:1234", "local-model");
    write_prompt(&dir, "default", "You are a helpful assistant.");
    write_policy(&dir, "default", &["local"], Some("default"));

    let config_dir = ConfigDir::new(&dir);
    let result = resolve_effective_config(&config_dir, None, Some("/tmp"));
    assert!(result.is_ok());
    let cfg = result.unwrap();
    assert_eq!(cfg.prompt, "You are a helpful assistant.");
}

#[test]
fn test_resolve_no_prompt_defaults_empty() {
    let env = TestEnv::new();
    let dir = env.dir();
    std::fs::create_dir_all(dir.join("connections")).unwrap();
    std::fs::create_dir_all(dir.join("policies")).unwrap();
    std::fs::create_dir_all(dir.join("prompts")).unwrap();

    write_config(&dir, "default");
    write_connection(&dir, "local", "openai", "http://localhost:1234", "local-model");
    write_policy(&dir, "default", &["local"], None);

    let config_dir = ConfigDir::new(&dir);
    let result = resolve_effective_config(&config_dir, None, Some("/tmp"));
    assert!(result.is_ok());
    let cfg = result.unwrap();
    assert!(cfg.prompt.is_empty());
}

#[test]
fn test_resolve_missing_prompt_fails() {
    let env = TestEnv::new();
    let dir = env.dir();
    std::fs::create_dir_all(dir.join("connections")).unwrap();
    std::fs::create_dir_all(dir.join("policies")).unwrap();
    std::fs::create_dir_all(dir.join("prompts")).unwrap();

    write_config(&dir, "default");
    write_connection(&dir, "local", "openai", "http://localhost:1234", "local-model");
    write_policy(&dir, "default", &["local"], Some("missing-prompt"));

    let config_dir = ConfigDir::new(&dir);
    let result = resolve_effective_config(&config_dir, None, Some("/tmp"));
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("missing-prompt"));
}

#[test]
fn test_resolve_empty_connections_fails() {
    let env = TestEnv::new();
    let dir = env.dir();
    std::fs::create_dir_all(dir.join("connections")).unwrap();
    std::fs::create_dir_all(dir.join("policies")).unwrap();
    std::fs::create_dir_all(dir.join("prompts")).unwrap();

    write_config(&dir, "default");
    let policy = serde_json::json!({ "connections": [] });
    std::fs::write(
        dir.join("policies").join("default.json"),
        policy.to_string(),
    )
    .unwrap();

    let config_dir = ConfigDir::new(&dir);
    let result = resolve_effective_config(&config_dir, None, Some("/tmp"));
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("connections"));
}

#[test]
fn test_resolve_invalid_root_policy_fails() {
    let env = TestEnv::new();
    let dir = env.dir();
    std::fs::create_dir_all(dir.join("connections")).unwrap();
    std::fs::create_dir_all(dir.join("policies")).unwrap();
    std::fs::create_dir_all(dir.join("prompts")).unwrap();

    let config = serde_json::json!({ "policy": "" });
    std::fs::write(dir.join("config.json"), config.to_string()).unwrap();

    let config_dir = ConfigDir::new(&dir);
    let result = resolve_effective_config(&config_dir, None, Some("/tmp"));
    assert!(result.is_err());
}

#[test]
fn test_resolve_missing_root_config_fails() {
    let env = TestEnv::new();
    let dir = env.dir();
    std::fs::create_dir_all(dir.join("connections")).unwrap();
    std::fs::create_dir_all(dir.join("policies")).unwrap();
    std::fs::create_dir_all(dir.join("prompts")).unwrap();

    let config_dir = ConfigDir::new(&dir);
    let result = resolve_effective_config(&config_dir, None, Some("/tmp"));
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("root config"));
}

#[test]
fn test_resolve_preserves_permissions() {
    let env = TestEnv::new();
    let dir = env.dir();
    std::fs::create_dir_all(dir.join("connections")).unwrap();
    std::fs::create_dir_all(dir.join("policies")).unwrap();
    std::fs::create_dir_all(dir.join("prompts")).unwrap();

    write_config(&dir, "default");
    write_connection(&dir, "local", "openai", "http://localhost:1234", "local-model");

    let policy = serde_json::json!({
        "connections": ["local"],
        "permissions": {
            "tools": ["bash", "fs_read", "fs_edit"],
            "paths": ["/tmp/**"]
        }
    });
    std::fs::write(
        dir.join("policies").join("default.json"),
        policy.to_string(),
    )
    .unwrap();

    let config_dir = ConfigDir::new(&dir);
    let result = resolve_effective_config(&config_dir, None, Some("/tmp"));
    assert!(result.is_ok());
    let cfg = result.unwrap();
    assert_eq!(cfg.permissions.tools, vec!["bash", "fs_read", "fs_edit"]);
    assert_eq!(cfg.permissions.paths, vec!["/tmp/**"]);
}

#[test]
fn test_resolve_preserves_limits() {
    let env = TestEnv::new();
    let dir = env.dir();
    std::fs::create_dir_all(dir.join("connections")).unwrap();
    std::fs::create_dir_all(dir.join("policies")).unwrap();
    std::fs::create_dir_all(dir.join("prompts")).unwrap();

    write_config(&dir, "default");
    write_connection(&dir, "local", "openai", "http://localhost:1234", "local-model");

    let policy = serde_json::json!({
        "connections": ["local"],
        "limits": {
            "token_warn_threshold": 80000,
            "token_hard_limit": 120000,
            "max_steps": 200
        }
    });
    std::fs::write(
        dir.join("policies").join("default.json"),
        policy.to_string(),
    )
    .unwrap();

    let config_dir = ConfigDir::new(&dir);
    let result = resolve_effective_config(&config_dir, None, Some("/tmp"));
    assert!(result.is_ok());
    let cfg = result.unwrap();
    assert_eq!(cfg.limits.token_warn_threshold, Some(80000));
    assert_eq!(cfg.limits.token_hard_limit, Some(120000));
    assert_eq!(cfg.limits.max_steps, Some(200));
}

#[test]
fn test_resolve_policy_hash_is_deterministic() {
    let env = TestEnv::new();
    let dir = env.dir();
    std::fs::create_dir_all(dir.join("connections")).unwrap();
    std::fs::create_dir_all(dir.join("policies")).unwrap();
    std::fs::create_dir_all(dir.join("prompts")).unwrap();

    write_config(&dir, "default");
    write_connection(&dir, "local", "openai", "http://localhost:1234", "local-model");
    write_policy(&dir, "default", &["local"], None);

    let config_dir = ConfigDir::new(&dir);
    let result1 = resolve_effective_config(&config_dir, None, Some("/tmp")).unwrap();
    let result2 = resolve_effective_config(&config_dir, None, Some("/tmp")).unwrap();
    assert_eq!(result1.policy_hash, result2.policy_hash);
    assert!(!result1.policy_hash.is_empty());
    assert!(!result1.policy_hash.contains("read_error"));
}

#[test]
fn test_resolve_invalid_connection_json_fails() {
    let env = TestEnv::new();
    let dir = env.dir();
    std::fs::create_dir_all(dir.join("connections")).unwrap();
    std::fs::create_dir_all(dir.join("policies")).unwrap();
    std::fs::create_dir_all(dir.join("prompts")).unwrap();

    write_config(&dir, "default");
    let conn = "{invalid json here";
    std::fs::write(
        dir.join("connections").join("local.json"),
        conn,
    )
    .unwrap();
    write_policy(&dir, "default", &["local"], None);

    let config_dir = ConfigDir::new(&dir);
    let result = resolve_effective_config(&config_dir, None, Some("/tmp"));
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("JSON"));
}

#[test]
fn test_resolve_connection_missing_required_fields() {
    let env = TestEnv::new();
    let dir = env.dir();
    std::fs::create_dir_all(dir.join("connections")).unwrap();
    std::fs::create_dir_all(dir.join("policies")).unwrap();
    std::fs::create_dir_all(dir.join("prompts")).unwrap();

    write_config(&dir, "default");
    let conn = serde_json::json!({ "base_url": "http://localhost:1234" });
    std::fs::write(
        dir.join("connections").join("bad.json"),
        conn.to_string(),
    )
    .unwrap();
    write_policy(&dir, "default", &["bad"], None);

    let config_dir = ConfigDir::new(&dir);
    let result = resolve_effective_config(&config_dir, None, Some("/tmp"));
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("interface"));
}
