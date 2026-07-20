use orchid::JsonError;
use serde_json;

mod support;

// Tests from src/jsonerr/mod.rs

// Original: test_serialize
// What it tests: Verifying that JsonError serializes correctly to JSON string,
// containing both the error code and the message text.
#[test]
fn test_serialize() {
    let err = JsonError::new("test_error", "test message");
    let json = serde_json::to_string(&err).unwrap();
    assert!(json.contains("test_error"));
    assert!(json.contains("test message"));
}

// Original: test_config_not_found
// What it tests: Verifying that the JsonError::config_not_found() constructor
// produces an error with the correct error code "config_not_found".
#[test]
fn test_config_not_found() {
    let err = JsonError::config_not_found();
    assert_eq!(err.error, "config_not_found");
}
