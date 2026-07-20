pub mod base;
pub use base::{is_retryable, BaseClient};
pub mod anthropic;
pub mod openai;
pub mod resolve;
pub mod sse;

use crate::config::{Connection, Profile};
use crate::provider::{Provider, ProviderError};
use std::sync::Arc;

pub use resolve::resolve_env_inline;

pub fn create_provider(profile: &Profile) -> Result<Arc<dyn Provider>, ProviderError> {
    create_provider_with_log(profile, None)
}

pub fn create_provider_from_connection(
    connection: &Connection,
) -> Result<Arc<dyn Provider>, ProviderError> {
    create_provider_from_connection_with_log(connection, None)
}

pub fn create_provider_from_connection_with_log(
    connection: &Connection,
    log_path: Option<std::path::PathBuf>,
) -> Result<Arc<dyn Provider>, ProviderError> {
    let profile = Profile {
        name: String::new(),
        provider: connection.interface.clone(),
        api_key: connection.api_key.clone().unwrap_or_default(),
        base_url: connection.base_url.clone(),
        model: connection.model.clone(),
        params: connection.params.clone(),
        headers: connection.headers.clone(),
        server_actions: std::collections::HashMap::new(),
        extra: std::collections::HashMap::new(),
        env: std::collections::HashMap::new(),
    };
    create_provider_with_log(&profile, log_path)
}

pub fn create_provider_with_log(
    profile: &Profile,
    log_path: Option<std::path::PathBuf>,
) -> Result<Arc<dyn Provider>, ProviderError> {
    let provider_name = if profile.provider.is_empty() {
        "anthropic"
    } else {
        &profile.provider
    };

    match provider_name {
        "anthropic" => {
            let mut client = anthropic::AnthropicClient::from_profile(profile)?;
            if let Some(path) = log_path {
                client = client.with_log(path);
            }
            Ok(Arc::new(client))
        }
        "openai" => {
            let mut client = openai::OpenAiClient::from_profile(profile)?;
            if let Some(path) = log_path {
                client = client.with_log(path);
            }
            Ok(Arc::new(client))
        }
        _ => Err(ProviderError::InvalidResponse(format!(
            "unknown provider: {}",
            provider_name
        ))),
    }
}
