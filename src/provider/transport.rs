use super::{Credential, HttpProvider};
use reqwest::blocking::{Client, Response};
use serde_json::Value;
use std::{io, time::Duration};

const RETRIES: usize = 2;

pub struct Transport {
    client: Client,
    credential: Option<Credential>,
}

impl Transport {
    pub fn new(credential: Option<Credential>, _log_level: String) -> io::Result<Self> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(120))
            .user_agent("codex_cli_rs/0.144.4 (orchid)")
            .build()
            .map_err(io::Error::other)?;
        Ok(Self { client, credential })
    }

    pub fn request(
        &self,
        provider: &HttpProvider,
        body: &Value,
        streaming: bool,
    ) -> io::Result<Response> {
        let request = self.request_for(provider, body);
        if streaming {
            return request.send().map_err(io::Error::other).and_then(status);
        }
        self.send_with_retries(request)
    }

    fn request_for(&self, provider: &HttpProvider, body: &Value) -> reqwest::blocking::RequestBuilder {
        let url = if matches!(self.credential, Some(Credential::Codex { .. }))
            && provider.connection.base_url.ends_with("/codex")
        {
            format!(
                "{}/responses",
                provider.connection.base_url.trim_end_matches('/')
            )
        } else {
            provider.connection.base_url.clone()
        };
        let mut request = self.client.post(url).json(body);
        if let Some(Credential::ApiKey(key)) = &self.credential {
            request = request.bearer_auth(key);
        }
        if let Some(Credential::Codex {
            access_token,
            account_id,
        }) = &self.credential
        {
            request = request
                .header("Authorization", format!("Bearer {access_token}"))
                .header("ChatGPT-Account-ID", account_id)
                .header("originator", "codex_cli_rs")
                .header("openai-beta", "responses=experimental")
                .header("Version", "0.144.4")
                .header("Session_Id", uuid::Uuid::new_v4().to_string());
        }
        for (name, value) in &provider.headers {
            request = request.header(name, value);
        }
        request
    }

    fn send_with_retries(&self, request: reqwest::blocking::RequestBuilder) -> io::Result<Response> {
        for attempt in 0..=RETRIES {
            let response = request
                .try_clone()
                .ok_or_else(|| io::Error::other("request setup failed"))?
                .send()
                .map_err(io::Error::other)
                .and_then(status);
            match response {
                Ok(response) => return Ok(response),
                Err(_error) if attempt < RETRIES => {
                    std::thread::sleep(Duration::from_millis(50 * (attempt + 1) as u64))
                }
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::other("transport request failed"))
    }
}

fn status(response: Response) -> io::Result<Response> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status().as_u16();
    let body = response
        .text()
        .unwrap_or_else(|error| format!("<unable to read error body: {error}>"));
    let body = body.trim();
    let body = if body.chars().count() > 2000 {
        format!("{}...", body.chars().take(2000).collect::<String>())
    } else {
        body.to_owned()
    };
    Err(io::Error::other(format!("HTTP status {status}: {body}")))
}
