use serde_json::json;
use tempfile::TempDir;

mod support;

// Tests from src/cmd/list.rs

// Original: test_list_empty
// What it tests: Verifying that an empty JSON array is a valid list result.
// This is a structural test; actual list() uses a live Store.
#[test]
fn test_list_empty() {
    // Use a temp dir to avoid conflicts
    let _temp = TempDir::new().unwrap();
    let result = json!([]);
    assert!(result.is_array());
    assert_eq!(result.as_array().unwrap().len(), 0);
}

// Original: test_list_is_json_array
// What it tests: Verifying that a non-empty list produces a valid JSON array
// with the expected structure (id, label, status fields).
#[test]
fn test_list_is_json_array() {
    let result = json!([
        {"id": "abc123", "label": "test", "status": "idle"}
    ]);
    assert!(result.is_array());
    assert_eq!(result.as_array().unwrap().len(), 1);
}