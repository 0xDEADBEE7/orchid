use orchid::client::create_provider_from_connections_with_log;
use orchid::config::Connection;
use orchid::provider::{ProviderError, Response};

fn connection(interface: &str) -> Connection {
    Connection {
        interface: interface.to_string(),
        base_url: "http://127.0.0.1:1".to_string(),
        api_key: None,
        auth: None,
        auth_profile: None,
        model: "test-model".to_string(),
        params: Default::default(),
        headers: Default::default(),
    }
}

#[test]
fn provider_fallback_skips_unknown_first_candidate() {
    let connections = vec![
        connection("unsupported-first"),
        connection("unsupported-second"),
    ];
    let error = match create_provider_from_connections_with_log(&connections, None) {
        Ok(_) => panic!("expected all candidates to fail"),
        Err(error) => error,
    };
    let message = error.to_string();
    assert!(message.contains("unknown provider: unsupported-first"));
    assert!(message.contains("unknown provider: unsupported-second"));
}

#[test]
fn provider_fallback_reports_empty_candidates() {
    let error = match create_provider_from_connections_with_log(&[], None) {
        Ok(_) => panic!("expected empty candidates to fail"),
        Err(error) => error,
    };
    assert!(error
        .to_string()
        .contains("all connection candidates failed"));
}

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
