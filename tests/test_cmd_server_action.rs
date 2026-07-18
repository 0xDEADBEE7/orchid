use orchid::cmd::server_action::{build_body, method_from_str, default_base_url};

mod support;

// Tests from src/cmd/server_action.rs

// Original: test_build_body_empty
// What it tests: Verifying that build_body() with an empty parameter list returns an
// empty JSON object.
#[test]
fn test_build_body_empty() {
    let body = build_body(&[]);
    assert!(body.as_object().unwrap().is_empty());
}

// Original: test_build_body_params
// What it tests: Verifying that build_body() correctly serializes key-value parameter
// pairs into a JSON object with the expected keys and values.
#[test]
fn test_build_body_params() {
    let params = vec![
        ("model".to_string(), "gpt-4".to_string()),
        ("n".to_string(), "3".to_string()),
    ];
    let body = build_body(&params);
    let obj = body.as_object().unwrap();
    assert_eq!(obj.get("model").unwrap(), "gpt-4");
    assert_eq!(obj.get("n").unwrap(), "3");
}

// Original: test_method_from_str
// What it tests: Verifying that method_from_str() correctly converts HTTP method strings
// (case-insensitive) to reqwest::Method variants, and rejects invalid strings.
#[test]
fn test_method_from_str() {
    assert_eq!(method_from_str("GET").unwrap(), reqwest::Method::GET);
    assert_eq!(method_from_str("post").unwrap(), reqwest::Method::POST);
    assert!(method_from_str("INVALID").is_err());
}

// Original: test_default_base_url
// What it tests: Verifying that default_base_url() returns the correct default API URL
// for known provider types (anthropic, openai-compat) and empty string for unknown.
#[test]
fn test_default_base_url() {
    assert_eq!(default_base_url("anthropic"), "https://api.anthropic.com");
    assert_eq!(default_base_url("openai-compat"), "https://api.openai.com");
    assert_eq!(default_base_url("unknown"), "");
}