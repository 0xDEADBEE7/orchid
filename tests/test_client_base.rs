use orchid::client::{BaseClient, is_retryable};

mod support;

// Tests from src/client/base.rs

// Original: test_is_retryable
// What it tests: Verifying that HTTP status codes 408, 429, 500, 502, 503, 504 are
// classified as retryable, while 400 and 401 are not.
#[test]
fn test_is_retryable() {
    assert!(is_retryable(408));
    assert!(is_retryable(429));
    assert!(is_retryable(500));
    assert!(is_retryable(502));
    assert!(is_retryable(503));
    assert!(is_retryable(504));
    assert!(!is_retryable(400));
    assert!(!is_retryable(401));
}

// Original: test_base_client_creation
// What it tests: Verifying that BaseClient::new() succeeds (basic constructor smoke test).
#[test]
fn test_base_client_creation() {
    let client = BaseClient::new();
    assert!(client.is_ok());
}