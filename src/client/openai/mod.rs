use crate::client::base::BaseClient;
use crate::provider::ProviderError;
use std::env;

const DEFAULT_API_URL: &str = "http://localhost:1234/v1/chat/completions";
const DEFAULT_MODEL: &str = "local-model";

mod wire;
mod messages;
mod tools;
mod sse;
mod api;

pub use wire::*;
pub use messages::to_openai_message;
pub use tools::openai_tool_definitions;
pub use sse::OpenAiStream;
pub use api::OpenAiApiClient;

pub struct OpenAiClient {
    base_client: BaseClient,
    api_url: String,
    model: String,
    max_tokens: u32,
    reasoning_effort: Option<String>,
    extra_headers: Vec<(String, String)>,
    auth_header: String,
}

impl OpenAiClient {
    pub fn new() -> Result<Self, ProviderError> {
        let _api_key = env::var("OPENAI_API_KEY").map_err(|_| {
            ProviderError::AuthError(
                "OPENAI_API_KEY environment variable not set".to_string(),
            )
        })?;

        let _base_client = BaseClient::new()?;

        Ok(OpenAiClient {
            base_client: BaseClient::new()?,
            api_url: DEFAULT_API_URL.to_string(),
            model: DEFAULT_MODEL.to_string(),
            max_tokens: 8192,
            reasoning_effort: None,
            extra_headers: vec![],
            auth_header: String::new(),
        })
    }

    pub fn from_profile(profile: &crate::config::Profile) -> Result<Self, ProviderError> {
        let raw_key = if profile.api_key.is_empty() {
            env::var("OPENAI_API_KEY").unwrap_or_default()
        } else if let Some(var) = profile.api_key.strip_prefix("env.") {
            env::var(var).unwrap_or_default()
        } else {
            profile.api_key.clone()
        };

        let extra_headers: Vec<(String, String)> = profile
            .headers
            .iter()
            .map(|(k, v)| {
                let resolved = crate::client::resolve::resolve_env_inline(v);
                (k.clone(), resolved)
            })
            .collect();

        let has_auth_header = extra_headers
            .iter()
            .any(|(k, _)| {
                k.eq_ignore_ascii_case("authorization") || k.eq_ignore_ascii_case("api-key")
            });

        if raw_key.is_empty() && !has_auth_header {
            return Err(ProviderError::AuthError(
                "no API key configured".to_string(),
            ));
        }

        let base_url = if profile.base_url.is_empty() {
            DEFAULT_API_URL.to_string()
        } else {
            format!(
                "{}/v1/chat/completions",
                profile.base_url.trim_end_matches('/')
            )
        };

        let model = if profile.model.is_empty() {
            DEFAULT_MODEL.to_string()
        } else {
            profile.model.clone()
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
            max_tokens: profile.max_tokens.unwrap_or(8192),
            reasoning_effort: profile.reasoning_effort.clone(),
            extra_headers,
            auth_header,
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


