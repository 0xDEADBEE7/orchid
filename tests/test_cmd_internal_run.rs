mod support;
use orchid::cmd::internal_run;

// Original: test_internal_run_unknown_profile
// What it tests: When internal_run is called with a profile name that doesn't
// exist in the config, it returns an error containing "not found" or "profile".
// This verifies error handling for missing profiles.
#[test]
fn test_internal_run_unknown_profile() {
    let env = support::TestEnv::new();
    let orchid_dir = env.dir();
    let config = serde_json::json!({
        "active_profile": "default",
        "profiles": {"default": {"provider": "anthropic", "api_key": "x", "model": "m"}}
    });
    std::fs::write(orchid_dir.join("config.json"), config.to_string()).unwrap();

    let err = internal_run("nonexistent_id", &Some("missing-profile".to_string())).unwrap_err();
    assert!(
        err.contains("not found") || err.contains("profile"),
        "got: {}",
        err
    );
}
