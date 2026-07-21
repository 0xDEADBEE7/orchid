mod support;
use orchid::config::resolve::{resolve as resolve_effective_config, EffectiveSessionConfig};
use orchid::config::{ConfigDir, Connection, Permissions, Policy, PolicyLimits, RootConfig};
use std::collections::HashMap;
use support::TestEnv;

fn write_config(dir: &std::path::Path, policy_name: &str) {
    let config = serde_json::json!({ "policy": policy_name });
    std::fs::write(dir.join("config.json"), config.to_string()).unwrap();
}

#[test]
fn resource_loaders_reject_unknown_fields() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("connections")).unwrap();
    std::fs::write(
        dir.path().join("config.json"),
        r#"{"policy":"default","unexpected":true}"#,
    )
    .unwrap();
    let error = orchid::ConfigDir::new(dir.path()).load_root().unwrap_err();
    assert!(error.to_string().contains("unknown field"));

    std::fs::write(
        dir.path().join("connections/local.json"),
        r#"{"interface":"openai","base_url":"http://localhost","model":"local","unexpected":true}"#,
    )
    .unwrap();
    let error = orchid::ConfigDir::new(dir.path())
        .load_connection("local")
        .unwrap_err();
    assert!(error.to_string().contains("unknown field"));
}

#[test]
fn nested_policy_objects_reject_unknown_fields() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("policies")).unwrap();
    std::fs::write(
        dir.path().join("policies/default.json"),
        r#"{"connections":["local"],"limits":{"max_steps":10,"unexpected":1}}"#,
    )
    .unwrap();
    let error = orchid::ConfigDir::new(dir.path())
        .load_policy("default")
        .unwrap_err();
    assert!(error.to_string().contains("unknown field"));
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

fn write_policy(dir: &std::path::Path, name: &str, connections: &[&str], prompt: Option<&str>) {
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
    write_connection(
        &dir,
        "local",
        "openai",
        "http://localhost:1234",
        "local-model",
    );
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
    write_connection(
        &dir,
        "fast",
        "openai",
        "http://localhost:1234",
        "fast-model",
    );
    write_connection(
        &dir,
        "smart",
        "openai",
        "http://localhost:1235",
        "smart-model",
    );
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
    write_connection(
        &dir,
        "local",
        "openai",
        "http://localhost:1234",
        "local-model",
    );
    write_prompt(&dir, "default", "You are a helpful assistant.");
    write_policy(&dir, "default", &["local"], Some("default"));

    let config_dir = ConfigDir::new(&dir);
    let result = resolve_effective_config(&config_dir, None, Some("/tmp"));
    assert!(result.is_ok());
    let cfg = result.unwrap();
    assert_eq!(cfg.prompt, "You are a helpful assistant.");
    assert_eq!(cfg.prompt_name.as_deref(), Some("default"));
}

#[test]
fn test_resolve_no_prompt_defaults_empty() {
    let env = TestEnv::new();
    let dir = env.dir();
    std::fs::create_dir_all(dir.join("connections")).unwrap();
    std::fs::create_dir_all(dir.join("policies")).unwrap();
    std::fs::create_dir_all(dir.join("prompts")).unwrap();

    write_config(&dir, "default");
    write_connection(
        &dir,
        "local",
        "openai",
        "http://localhost:1234",
        "local-model",
    );
    write_policy(&dir, "default", &["local"], None);

    let config_dir = ConfigDir::new(&dir);
    let result = resolve_effective_config(&config_dir, None, Some("/tmp"));
    assert!(result.is_ok());
    let cfg = result.unwrap();
    assert!(cfg.prompt.is_empty());
    assert_eq!(cfg.prompt_name, None);
}

#[test]
fn test_resolve_missing_prompt_fails() {
    let env = TestEnv::new();
    let dir = env.dir();
    std::fs::create_dir_all(dir.join("connections")).unwrap();
    std::fs::create_dir_all(dir.join("policies")).unwrap();
    std::fs::create_dir_all(dir.join("prompts")).unwrap();

    write_config(&dir, "default");
    write_connection(
        &dir,
        "local",
        "openai",
        "http://localhost:1234",
        "local-model",
    );
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
fn test_policy_environment_resolves_without_leaking_missing_values() {
    let env = TestEnv::new();
    let dir = env.dir();
    std::fs::create_dir_all(dir.join("connections")).unwrap();
    std::fs::create_dir_all(dir.join("policies")).unwrap();
    std::fs::create_dir_all(dir.join("prompts")).unwrap();

    write_config(&dir, "default");
    write_connection(
        &dir,
        "local",
        "openai",
        "http://localhost:1234",
        "local-model",
    );
    let missing = "ORCHID_POLICY_MISSING_REDACTION_TEST";
    std::env::remove_var(missing);
    let policy = serde_json::json!({
        "connections": ["local"],
        "env": {"TOKEN": format!("Bearer env.{}", missing)}
    });
    std::fs::write(dir.join("policies/default.json"), policy.to_string()).unwrap();

    let error = resolve_effective_config(&ConfigDir::new(&dir), None, Some("/tmp")).unwrap_err();
    assert!(error.contains(missing));
    assert!(!error.contains("Bearer"));
}

#[test]
fn test_policy_environment_is_runtime_only() {
    let env = TestEnv::new();
    let dir = env.dir();
    std::fs::create_dir_all(dir.join("connections")).unwrap();
    std::fs::create_dir_all(dir.join("policies")).unwrap();
    std::fs::create_dir_all(dir.join("prompts")).unwrap();

    write_config(&dir, "default");
    write_connection(
        &dir,
        "local",
        "openai",
        "http://localhost:1234",
        "local-model",
    );
    let secret = "policy-runtime-secret";
    std::env::set_var("ORCHID_POLICY_SECRET_TEST", secret);
    let policy = serde_json::json!({
        "connections": ["local"],
        "env": {"TOKEN": "env.ORCHID_POLICY_SECRET_TEST"}
    });
    let policy_path = dir.join("policies/default.json");
    std::fs::write(&policy_path, policy.to_string()).unwrap();

    let effective = resolve_effective_config(&ConfigDir::new(&dir), None, Some("/tmp")).unwrap();
    assert_eq!(effective.env_vars.get("TOKEN"), Some(&secret.to_string()));
    let serialized = format!("policy={} env_vars=<runtime-only>", effective.policy_name);
    assert!(!serialized.contains(secret));
    assert_eq!(effective.env_vars.get("TOKEN"), Some(&secret.to_string()));
    assert_eq!(
        std::fs::read_to_string(policy_path).unwrap(),
        policy.to_string()
    );
    std::env::remove_var("ORCHID_POLICY_SECRET_TEST");
}
#[test]
fn test_resolve_preserves_permissions() {
    let env = TestEnv::new();
    let dir = env.dir();
    std::fs::create_dir_all(dir.join("connections")).unwrap();
    std::fs::create_dir_all(dir.join("policies")).unwrap();
    std::fs::create_dir_all(dir.join("prompts")).unwrap();

    write_config(&dir, "default");
    write_connection(
        &dir,
        "local",
        "openai",
        "http://localhost:1234",
        "local-model",
    );

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
fn session_path_restrictions_cannot_expand_policy_paths() {
    let policy = orchid::config::Permissions {
        tools: vec!["fs_read".into()],
        paths: vec!["/tmp/project".into()],
    };
    let narrowed = orchid::config::resolve::intersect_permissions(
        &policy,
        Some(&["/tmp/project/src".into()]),
    );
    assert_eq!(narrowed.tools, policy.tools);
    assert_eq!(narrowed.paths, vec!["/tmp/project/src"]);

    let outside = orchid::config::resolve::intersect_permissions(
        &policy,
        Some(&["/etc".into()]),
    );
    assert!(outside.paths.is_empty());
}

#[test]
fn session_restrictions_match_glob_policy_paths() {
    let policy = orchid::config::Permissions {
        tools: vec!["fs_read".into()],
        paths: vec!["/tmp/**".into()],
    };
    let narrowed = orchid::config::resolve::intersect_permissions(
        &policy,
        Some(&["/tmp/project/src".into()]),
    );
    assert_eq!(narrowed.paths, vec!["/tmp/project/src"]);
}

#[test]
fn test_resolve_preserves_limits() {
    let env = TestEnv::new();
    let dir = env.dir();
    std::fs::create_dir_all(dir.join("connections")).unwrap();
    std::fs::create_dir_all(dir.join("policies")).unwrap();
    std::fs::create_dir_all(dir.join("prompts")).unwrap();

    write_config(&dir, "default");
    write_connection(
        &dir,
        "local",
        "openai",
        "http://localhost:1234",
        "local-model",
    );

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
    write_connection(
        &dir,
        "local",
        "openai",
        "http://localhost:1234",
        "local-model",
    );
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
    std::fs::write(dir.join("connections").join("local.json"), conn).unwrap();
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
    std::fs::write(dir.join("connections").join("bad.json"), conn.to_string()).unwrap();
    write_policy(&dir, "default", &["bad"], None);

    let config_dir = ConfigDir::new(&dir);
    let result = resolve_effective_config(&config_dir, None, Some("/tmp"));
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("interface"));
}
