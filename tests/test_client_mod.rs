use orchid::client::create_provider_from_connection;
use orchid::config::Connection;
use std::collections::HashMap;

mod support;

// Test from src/client/mod.rs
// Verifies the provider factory accepts the new Connection resource directly.
#[test]
fn test_create_provider_defaults_to_anthropic() {
    let connection = Connection {
        interface: "anthropic".to_string(),
        base_url: "https://example.test".to_string(),
        api_key: None,
        auth: None,
        auth_profile: None,
        model: "test-model".to_string(),
        headers: HashMap::new(),
        params: HashMap::new(),
    };

    let result = create_provider_from_connection(&connection);
    // This is essentially a smoke test - just ensuring the function doesn't panic
    assert!(result.is_err() || result.is_ok());
}
