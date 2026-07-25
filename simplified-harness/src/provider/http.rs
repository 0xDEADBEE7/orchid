use super::{codex_headers, HttpProvider};
use serde_json::{json, Value};
use std::io;

pub fn reply(provider: &HttpProvider, prompt: &str) -> io::Result<String> {
    let codex = is_codex(provider);
    let body = body(provider, prompt, codex);
    let response = send(provider, body, codex)?;
    text(response)
}

fn is_codex(provider: &HttpProvider) -> bool {
    provider.connection.auth.as_deref() == Some("openai-codex")
        || provider
            .connection
            .base_url
            .contains("chatgpt.com/backend-api/codex")
}

fn body(provider: &HttpProvider, prompt: &str, codex: bool) -> Value {
    match (codex, provider.connection.interface.as_str()) {
        (true, _) => {
            json!({"model":provider.connection.model,"instructions":"You are Orchid, a helpful coding assistant.","input":[{"role":"user","content":[{"type":"input_text","text":prompt}]}],"store":false})
        }
        (false, "anthropic") => {
            json!({"model":provider.connection.model,"max_tokens":1024,"messages":[{"role":"user","content":prompt}]})
        }
        _ => {
            json!({"model":provider.connection.model,"messages":[{"role":"user","content":prompt}]})
        }
    }
}

fn send(provider: &HttpProvider, body: Value, codex: bool) -> io::Result<Value> {
    let url = if codex && provider.connection.base_url.ends_with("/codex") {
        format!("{}/responses", provider.connection.base_url)
    } else {
        provider.connection.base_url.clone()
    };
    let mut request = reqwest::blocking::Client::new().post(url).json(&body);
    if let Some(key) = &provider.connection.api_key {
        request = request.bearer_auth(key);
    }
    if codex {
        request = codex_headers(
            request,
            &provider.root,
            provider
                .connection
                .auth
                .as_deref()
                .unwrap_or("openai-codex"),
        )?;
    }
    for attempt in 0..3 {
        match request
            .try_clone()
            .ok_or_else(|| io::Error::other("request cannot be cloned"))?
            .send()
            .and_then(|r| r.error_for_status())
        {
            Ok(response) => return response.json().map_err(io::Error::other),
            Err(error) if attempt < 2 => {
                let _ = error;
                std::thread::sleep(std::time::Duration::from_millis(50 * (attempt + 1)));
            }
            Err(error) => return Err(io::Error::other(error)),
        }
    }
    Err(io::Error::other("provider request failed"))
}

fn text(value: Value) -> io::Result<String> {
    value
        .pointer("/choices/0/message/content")
        .or_else(|| value.pointer("/content/0/text"))
        .or_else(|| value.pointer("/output/0/content/0/text"))
        .or_else(|| value.pointer("/output_text"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| io::Error::other("provider response has no text"))
}
