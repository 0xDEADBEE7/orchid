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

#[test]
#[serial_test::serial]
fn test_create_resolves_policy_before_creating_session() {
    let env = TestEnv::new();
    let dir = env.dir();
    write_resource_config(&dir);
    std::fs::remove_file(dir.join("connections/local.json")).unwrap();

    let result = orchid::cmd::create(
        None,
        Some("/tmp".to_string()),
        None,
        Some("default".to_string()),
        None,
        &dir,
    );
    assert!(result.is_err());
    let entries: Vec<_> = std::fs::read_dir(dir.join("sessions"))
        .unwrap()
        .filter_map(Result::ok)
        .collect();
    assert!(entries.iter().all(|entry| !entry.path().is_dir()));
}

#[test]
#[serial_test::serial]
fn test_send_missing_secret_has_no_transcript_side_effect() {
    let env = TestEnv::new();
    let dir = env.dir();
    write_resource_config(&dir);
    std::env::remove_var("ORCHID_MISSING_MVP_KEY");
    std::fs::write(
        dir.join("connections/local.json"),
        r#"{"interface":"openai","base_url":"http://127.0.0.1:9","model":"test","api_key":"env.ORCHID_MISSING_MVP_KEY"}"#,
    )
    .unwrap();

    let before: Vec<_> = std::fs::read_dir(dir.join("sessions"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    let result = send(
        None,
        "secret-free failure".to_string(),
        true,
        &dir,
        None,
        Some("/tmp".to_string()),
        Some("default".to_string()),
        None,
    );
    assert!(result.is_err());
    let after: Vec<_> = std::fs::read_dir(dir.join("sessions"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    assert_eq!(after, before);
}
#[test]
#[serial_test::serial]
fn test_fork_errors_when_no_policy_connection_available() {
    let env = TestEnv::new();
    let orchid_dir = env.dir();
    write_resource_config(orchid_dir.as_path());
    let config_dir = orchid_dir.clone();
    std::fs::remove_file(config_dir.join("connections/local.json")).unwrap();
    let store = Store::with_config_dir(&config_dir).unwrap();
    let meta = store.create(None, Some("/tmp".to_string()), None).unwrap();
    let result = send(
        Some(meta.id.clone()),
        "test".to_string(),
        false,
        &config_dir,
        None,
        None,
        None,
        None,
    );
    assert!(
        result.is_err(),
        "should error when no policy connection is available"
    );
    let error = result.unwrap_err();
    assert!(error.contains("connection") || error.contains("policy"));
}

#[test]
#[serial_test::serial]
fn test_send_writes_user_message_to_session_jsonl() {
    let env = TestEnv::new();
    let orchid_dir = env.dir();
    write_resource_config(orchid_dir.as_path());
    let config_dir = orchid_dir.clone();
    let store = Store::with_config_dir(&config_dir).unwrap();
    let meta = store.create(None, Some("/tmp".to_string()), None).unwrap();
    let send_result = send(
        Some(meta.id.clone()),
        "hello world".to_string(),
        false,
        &config_dir,
        None,
        None,
        Some("default".to_string()),
        None,
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
        .join("conversation.jsonl");
    assert!(
        std::fs::read_to_string(&jsonl)
            .map(|c| c.contains("hello world"))
            .unwrap_or(false),
        "user message should be written to the transcript"
    );
}
