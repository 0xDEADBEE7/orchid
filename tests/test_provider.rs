use orchid::provider::{Response, ProviderError};

mod support;

// Tests from src/provider/mod.rs

// Original: test_response_serialize
// What it tests: Verifying that Response serializes to JSON containing the message
// content while omitting null fields (like tool_calls when None).
#[test]
fn test_response_serialize() {
    let resp = Response {
        message: Some("hello".to_string()),
        reasoning: None,
        tool_calls: None,
        usage: None,
        model: None,
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("hello"));
    assert!(!json.contains("tool_calls"));
}

// Original: test_provider_error_display
// What it tests: Verifying that ProviderError::Network formats its display string
// correctly as "network error: <message>".
#[test]
fn test_provider_error_display() {
    let err = ProviderError::Network("connection failed".to_string());
    assert_eq!(err.to_string(), "network error: connection failed");
}