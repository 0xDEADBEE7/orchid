use crate::client::base::BaseClient;
use crate::config::Connection;
use crate::provider::ProviderError;

use std::env;

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const DEFAULT_MODEL: &str = "claude-3-5-sonnet-20241022";

mod api;
mod messages;
mod sse;
mod wire;

pub use crate::provider::StreamEvent;
pub use api::AnthropicApiClient;
pub use messages::to_wire_message;
pub use sse::SseStream;
pub use wire::*;

pub struct AnthropicClient {
    base_client: BaseClient,
    api_url: String,
    api_key: String,
    model: String,
    extra_headers: Vec<(String, String)>,
    params: Vec<(String, serde_json::Value)>,
}

impl AnthropicClient {
    pub fn new() -> Result<Self, ProviderError> {
        let api_key = env::var("ANTHROPIC_API_KEY").map_err(|_| {
            ProviderError::AuthError("ANTHROPIC_API_KEY environment variable not set".to_string())
        })?;

        let base_client = BaseClient::new()?;

        Ok(AnthropicClient {
            base_client,
            api_url: API_URL.to_string(),
            api_key,
            model: DEFAULT_MODEL.to_string(),
            extra_headers: vec![],
            params: vec![],
        })
    }

    pub fn from_connection(connection: &Connection) -> Result<Self, ProviderError> {
        let resolved = crate::client::resolve::resolve_connection(connection)
            .map_err(|e| ProviderError::AuthError(e.to_string()))?;
        let raw_key = match (&connection.auth_profile, resolved.api_key) {
            (Some(profile), _) => crate::client::resolve::resolve_auth(profile)
                .map_err(|e| ProviderError::AuthError(e.to_string()))?,
            (_, Some(key)) => key,
            (_, None) => env::var("ANTHROPIC_API_KEY").map_err(|_| {
                ProviderError::AuthError(
                    "ANTHROPIC_API_KEY environment variable not set".to_string(),
                )
            })?,
        };

        let extra_headers: Vec<(String, String)> = resolved.headers.into_iter().collect();

        let has_auth_header = extra_headers.iter().any(|(k, _)| {
            k.eq_ignore_ascii_case("authorization") || k.eq_ignore_ascii_case("x-api-key")
        });

        if raw_key.is_empty() && !has_auth_header {
            return Err(ProviderError::AuthError(
                "no API key configured".to_string(),
            ));
        }
        let base_url = if connection.base_url.is_empty() {
            API_URL.to_string()
        } else {
            format!("{}/v1/messages", connection.base_url.trim_end_matches('/'))
        };

        let model = if connection.model.is_empty() {
            DEFAULT_MODEL.to_string()
        } else {
            connection.model.clone()
        };

        Ok(AnthropicClient {
            base_client: BaseClient::new()?,
            api_url: base_url,
            api_key: raw_key,
            model,
            extra_headers,
            params: resolved.params.into_iter().collect(),
        })
    }

    pub fn with_log(mut self, path: std::path::PathBuf) -> Self {
        self.base_client = self.base_client.with_log(path);
        self
    }

    pub fn api_client(&self) -> AnthropicApiClient<'_> {
        AnthropicApiClient { inner: self }
    }
}
