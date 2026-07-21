mod support;
use orchid::cmd::internal_run;
use support::TestEnv;

// Updated: test_internal_run_missing_policy
// Tests that internal_run with the new config model fails when the config
// directory has no valid root config (no policy field).
#[test]
fn test_internal_run_uses_selected_config_directory() {
    let env = TestEnv::new();
    let selected = env.dir().join("selected");
    let other = env.dir().join("other");
    std::fs::create_dir_all(&selected).unwrap();
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(selected.join("config.json"), r#"{"policy":"missing"}"#).unwrap();
    std::fs::write(other.join("config.json"), r#"{"policy":"other"}"#).unwrap();

    let _error =
        orchid::cmd::internal_run("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", &selected).unwrap_err();
    assert!(!other
        .join("sessions")
        .join("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .exists());
    assert!(!other
        .join("sessions")
        .join("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .exists());
}
#[test]
fn test_internal_run_missing_policy() {
    let env = support::TestEnv::new();
    let orchid_dir = env.dir();
    let config = serde_json::json!({});
    std::fs::write(orchid_dir.join("config.json"), config.to_string()).unwrap();

    let err = internal_run("nonexistent_id", &orchid_dir).unwrap_err();
    assert!(
        err.contains("policy") || err.contains("root config") || err.contains("missing"),
        "got: {}",
        err
    );
}

#[test]
fn test_internal_run_missing_config_dir() {
    let env = support::TestEnv::new();
    let orchid_dir = env.dir();
    let err = internal_run("nonexistent_id", &orchid_dir).unwrap_err();
    assert!(
        err.contains("root config") || err.contains("missing"),
        "got: {}",
        err
    );
}
