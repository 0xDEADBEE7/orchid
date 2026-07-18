pub mod base;
pub use base::{BaseClient, is_retryable};
pub mod resolve;
pub mod sse;
pub mod anthropic;
pub mod openai;

use crate::config::Profile;
use crate::provider::{Provider, ProviderError};
use std::sync::Arc;

pub use resolve::resolve_env_inline;

pub fn create_provider(profile: &Profile) -> Result<Arc<dyn Provider>, ProviderError> {
    create_provider_with_log(profile, None)
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
