use orchid::cli::{print_json, print_error};
use orchid::JsonError;
use serde_json::json;

mod support;

#[test]
fn test_print_json() {
    let value = json!({"key": "value"});
    let result = print_json(&value);
    assert!(result.is_ok());
}

#[test]
fn test_print_error() {
    let err = JsonError::config_not_found();
    let result = print_error(&err);
    assert!(result.is_ok());
}
