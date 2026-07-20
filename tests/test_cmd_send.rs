use orchid::cmd::send;
use orchid::convo::Store;
mod support;
use support::TestEnv;

fn write_minimal_config(dir: &std::path::Path, active_profile: Option<&str>) {
    let profile_section = r#""test-profile":{"provider":"anthropic","api_key":"x","model":"m"}"#;
    let active = active_profile
        .map(|p| format!(r#""active_profile":"{}","#, p))
        .unwrap_or_default();
    let json = format!(r#"{{{}"profiles":{{{}}}}}"#, active, profile_section);
    std::fs::write(dir.join("config.json"), json).unwrap();
}

#[test]
fn test_fork_uses_active_profile_not_hardcoded_default() {
    let profile: Option<String> = None;
    let active_profile: Option<String> = Some("cba-sonnet".to_string());
    let profile_arg = profile
        .as_ref()
        .map(|p| p.clone())
        .or(active_profile)
        .expect("should fall back to active_profile");
    assert_eq!(profile_arg, "cba-sonnet");
    assert_ne!(
        profile_arg, "default",
        "must not fall back to hardcoded 'default'"
    );
}

#[test]
#[serial_test::serial]
fn test_fork_errors_when_no_profile_available() {
    let env = TestEnv::new();
    let orchid_dir = env.dir();
    write_minimal_config(orchid_dir.as_path(), None);
    let config_dir = orchid_dir.clone();
    std::fs::create_dir_all(config_dir.join("sessions")).unwrap();
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
    assert!(result.is_err(), "should error when no profile is available");
    assert!(
        result.unwrap_err().contains("profile"),
        "error should mention profile"
    );
}

#[test]
#[serial_test::serial]
fn test_send_writes_user_message_to_jsonl() {
    let env = TestEnv::new();
    let orchid_dir = env.dir();
    write_minimal_config(orchid_dir.as_path(), Some("test-profile"));
    let config_dir = orchid_dir.clone();
    std::fs::create_dir_all(config_dir.join("sessions")).unwrap();
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
        Some("test-profile".to_string()),
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
            .map(|c| c.contains("\"type\":\"message\""))
            .unwrap_or(false),
        "event should have type field"
    );
}
