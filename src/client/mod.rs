pub mod base;
pub use base::{is_retryable, BaseClient};
pub mod anthropic;
pub mod openai;
pub mod resolve;
pub mod sse;

use crate::config::Connection;
use crate::provider::{Provider, ProviderError};
use std::sync::Arc;

pub use resolve::resolve_env_inline;

pub fn create_provider_from_connection(
    connection: &Connection,
) -> Result<Arc<dyn Provider>, ProviderError> {
    create_provider_from_connection_with_log(connection, None)
}

pub fn create_provider_from_connection_with_log(
    connection: &Connection,
    log_path: Option<std::path::PathBuf>,
) -> Result<Arc<dyn Provider>, ProviderError> {
    let provider_name = if connection.interface.is_empty() {
        "anthropic"
    } else {
        &connection.interface
    };

    match provider_name {
        "anthropic" => {
            let mut client = anthropic::AnthropicClient::from_connection(connection)?;
            if let Some(path) = log_path {
                client = client.with_log(path);
            }
            Ok(Arc::new(client))
        }
        "openai" => {
            let mut client = openai::OpenAiClient::from_connection(connection)?;
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
