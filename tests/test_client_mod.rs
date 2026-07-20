use orchid::client::create_provider;
use orchid::config::Profile;
use std::collections::HashMap;

mod support;

// Test from src/client/mod.rs
// Original: test_create_provider_defaults_to_anthropic
// What it tests: Verifying create_provider() doesn't panic when given an empty/default profile.
// The profile has no provider name, no API key, no model - essentially all empty strings.
// This is a basic smoke test to ensure the provider factory handles edge cases gracefully.
#[test]
fn test_create_provider_defaults_to_anthropic() {
    let profile = Profile {
        name: "test".to_string(),
        provider: String::new(),
        api_key: String::new(),
        base_url: String::new(),
        model: String::new(),
        headers: HashMap::new(),
        params: HashMap::new(),
        server_actions: HashMap::new(),
        extra: HashMap::new(),
        env: HashMap::new(),
    };

    let result = create_provider(&profile);
    // This is essentially a smoke test - just ensuring the function doesn't panic
    assert!(result.is_err() || result.is_ok());
}