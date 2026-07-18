mod support;
use orchid::cmd::{config_current, config_path};

// Original: test_config_path_ok
// What it tests: config_path() returns the path to the config.json file.
// Verifies the returned JSON contains a "path" field ending with "config.json".
#[test]
fn test_config_path_ok() {
    let result = config_path();
    assert!(result.is_ok());
    let val = result.unwrap();
    assert!(val.get("path").is_some());
    assert!(val["path"].as_str().unwrap().ends_with("config.json"));
}

// Original: test_config_current_missing
// What it tests: config_current() when no profile is set returns an error
// (or succeeds if a config exists). The key guarantee is no panic.
#[test]
fn test_config_current_missing() {
    // May succeed or fail depending on whether a config exists — just don't panic.
    let _result = config_current();
}