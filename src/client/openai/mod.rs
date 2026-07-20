use crate::client::base::BaseClient;
use crate::config::Connection;
use crate::provider::ProviderError;
use std::env;

const DEFAULT_API_URL: &str = "http://localhost:1234/v1/chat/completions";
const DEFAULT_MODEL: &str = "local-model";

mod api;
mod messages;
mod sse;
mod tools;
mod wire;

pub use api::OpenAiApiClient;
pub use messages::to_openai_message;
pub use sse::OpenAiStream;
pub use tools::openai_tool_definitions;
pub use wire::*;

pub struct OpenAiClient {
    base_client: BaseClient,
    api_url: String,
    model: String,
    extra_headers: Vec<(String, String)>,
    auth_header: String,
    params: Vec<(String, serde_json::Value)>,
}

impl OpenAiClient {
    pub fn new() -> Result<Self, ProviderError> {
        let _api_key = env::var("OPENAI_API_KEY").map_err(|_| {
            ProviderError::AuthError("OPENAI_API_KEY environment variable not set".to_string())
        })?;

        let _base_client = BaseClient::new()?;

        Ok(OpenAiClient {
            base_client: BaseClient::new()?,
            api_url: DEFAULT_API_URL.to_string(),
            model: DEFAULT_MODEL.to_string(),
            extra_headers: vec![],
            auth_header: String::new(),
            params: vec![],
        })
    }

    pub fn from_connection(connection: &Connection) -> Result<Self, ProviderError> {
        let raw_key = match &connection.api_key {
            Some(key) => crate::client::resolve::resolve_env_inline_strict(key)
                .map_err(|e| ProviderError::AuthError(e.to_string()))?,
            None => env::var("OPENAI_API_KEY").map_err(|_| {
                ProviderError::AuthError("OPENAI_API_KEY environment variable not set".to_string())
            })?,
        };

        let extra_headers: Vec<(String, String)> = connection
            .headers
            .iter()
            .map(|(k, v)| {
                crate::client::resolve::resolve_env_inline_strict(v)
                    .map(|resolved| (k.clone(), resolved))
                    .map_err(|e| ProviderError::AuthError(e.to_string()))
            })
            .collect::<Result<_, _>>()?;

        let has_auth_header = extra_headers.iter().any(|(k, _)| {
            k.eq_ignore_ascii_case("authorization") || k.eq_ignore_ascii_case("api-key")
        });

        if raw_key.is_empty() && !has_auth_header {
            return Err(ProviderError::AuthError(
                "no API key configured".to_string(),
            ));
        }

        let base_url = if connection.base_url.is_empty() {
            DEFAULT_API_URL.to_string()
        } else {
            format!(
                "{}/v1/chat/completions",
                connection.base_url.trim_end_matches('/')
            )
        };

        let model = if connection.model.is_empty() {
            DEFAULT_MODEL.to_string()
        } else {
            connection.model.clone()
        };

        let auth_header = if raw_key.is_empty() && has_auth_header {
            String::new()
        } else {
            format!("Bearer {}", raw_key)
        };

        Ok(OpenAiClient {
            base_client: BaseClient::new()?,
            api_url: base_url,
            model,
            extra_headers,
            auth_header,
            params: connection
                .params
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        })
    }

    pub fn with_log(mut self, path: std::path::PathBuf) -> Self {
        self.base_client = self.base_client.with_log(path);
        self
    }

    pub fn api_client(&self) -> OpenAiApiClient<'_> {
        OpenAiApiClient { inner: self }
    }
}
