use orchid::cmd::send;
use orchid::SessionStore as Store;
mod support;
use support::TestEnv;

fn write_resource_config(dir: &std::path::Path) {
    for name in ["connections", "policies", "prompts", "sessions"] {
        std::fs::create_dir_all(dir.join(name)).unwrap();
    }
    std::fs::write(dir.join("config.json"), r#"{"policy":"default"}"#).unwrap();
    std::fs::write(
        dir.join("connections/local.json"),
        r#"{"interface":"anthropic","base_url":"http://127.0.0.1:9","model":"test-model"}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("policies/default.json"),
        r#"{"connections":["local"]}"#,
    )
    .unwrap();
}

#[serial_test::serial]
fn test_fork_errors_when_no_policy_connection_available() {
    let env = TestEnv::new();
    let orchid_dir = env.dir();
    write_resource_config(orchid_dir.as_path());
    let config_dir = orchid_dir.clone();
    std::fs::remove_file(config_dir.join("connections/local.json")).unwrap();
    let store = Store::with_config_dir(&config_dir).unwrap();
    let meta = store
        .create(None, Some("/tmp".to_string()), None, None, None)
        .unwrap();
    let result = send(
        Some(meta.id.clone()),
        "test".to_string(),
        false,
        &config_dir,
        None,
        None,
        None,
    );
    assert!(
        result.is_err(),
        "should error when no policy connection is available"
    );
    let error = result.unwrap_err();
    assert!(
        error.contains("connection") || error.contains("policy"),
        "error should mention the missing resource"
    );
}

#[test]
#[serial_test::serial]
fn test_send_writes_user_message_to_session_jsonl() {
    let env = TestEnv::new();
    let orchid_dir = env.dir();
    write_resource_config(orchid_dir.as_path());
    let config_dir = orchid_dir.clone();
    let store = Store::with_config_dir(&config_dir).unwrap();
    let meta = store
        .create(None, Some("/tmp".to_string()), None, None, None)
        .unwrap();
    let send_result = send(
        Some(meta.id.clone()),
        "hello world".to_string(),
        false,
        &config_dir,
        None,
        None,
        Some("default".to_string()),
    );
    if let Err(ref e) = send_result {
        assert!(
            e.contains("spawn") || e.contains("fork") || e.contains("failed to spawn"),
            "unexpected send error: {}",
            e
        );
    }
    let jsonl = orchid_dir
        .join("sessions")
        .join(&meta.id)
        .join("session.jsonl");
    assert!(
        std::fs::read_to_string(&jsonl)
            .map(|c| c.contains("hello world"))
            .unwrap_or(false),
        "user message should be written to the transcript"
    );
}
