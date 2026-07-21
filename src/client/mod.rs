pub mod base;
pub use base::{is_retryable, BaseClient};
pub mod anthropic;
pub mod codex;
pub mod openai;
pub mod resolve;
pub mod sse;

use crate::config::Connection;
use crate::provider::{Provider, ProviderError};
use std::sync::Arc;

pub use resolve::{
    resolve_connection, resolve_env_inline_strict, EnvResolutionError, ResolvedConnection,
};

pub fn create_provider_from_connections_with_log(
    connections: &[Connection],
    log_path: Option<std::path::PathBuf>,
) -> Result<Arc<dyn Provider>, ProviderError> {
    let mut diagnostics = Vec::new();
    for connection in connections {
        match create_provider_from_connection_with_log(connection, log_path.clone()) {
            Ok(provider) => return Ok(provider),
            Err(error) => diagnostics.push(format!("{}: {}", connection.interface, error)),
        }
    }
    Err(ProviderError::InvalidResponse(format!(
        "all connection candidates failed: {}",
        diagnostics.join("; ")
    )))
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
            if connection.auth_profile.as_ref().map(|p| p.kind.as_str())
                == Some("openai_codex_oauth")
            {
                return Ok(Arc::new(codex::CodexClient::from_connection(connection)?));
            }
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
