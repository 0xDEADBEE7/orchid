use super::{codex_curl, parse_sse, tool_definitions, HttpProvider, Session, StreamEvent};
use serde_json::json;
use std::io::{self, Read};

pub fn run(
    provider: &HttpProvider,
    prompt: &str,
    _session: &Session,
) -> io::Result<Vec<StreamEvent>> {
    if provider.connection.auth.as_deref() == Some("openai-codex")
        || provider
            .connection
            .base_url
            .contains("chatgpt.com/backend-api/codex")
    {
        let url = if provider.connection.base_url.ends_with("/codex") {
            format!("{}/responses", provider.connection.base_url)
        } else {
            provider.connection.base_url.clone()
        };
        let body = json!({"model":provider.connection.model,"instructions":"You are Orchid, a helpful coding assistant.","input":[{"role":"user","content":[{"type":"input_text","text":prompt}]}],"tools":codex_tools(&provider.tools),"store":false,"stream":true});
        return codex_curl(
            &url,
            &body,
            &provider.root,
            provider
                .connection
                .auth
                .as_deref()
                .unwrap_or("openai-codex"),
        )
        .map(|raw| parse_sse(&raw));
    }
    let body = if provider.connection.interface == "anthropic" {
        json!({"model":provider.connection.model,"max_tokens":1024,"stream":true,"messages":[{"role":"user","content":prompt}]})
    } else {
        json!({"model":provider.connection.model,"stream":true,"messages":[{"role":"user","content":prompt}],"tools":tool_definitions(&provider.tools)})
    };
    let mut request = reqwest::blocking::Client::new()
        .post(&provider.connection.base_url)
        .json(&body);
    if let Some(key) = &provider.connection.api_key {
        request = request.bearer_auth(key);
    }
    let mut text = String::new();
    request
        .send()
        .map_err(io::Error::other)?
        .error_for_status()
        .map_err(io::Error::other)?
        .read_to_string(&mut text)
        .map_err(io::Error::other)?;
    Ok(parse_sse(&text))
}

fn codex_tools(enabled: &[String]) -> Vec<serde_json::Value> {
    tool_definitions(enabled).into_iter().filter_map(|tool| { let function = tool.get("function")?; Some(json!({"type":"function","name":function["name"],"description":function["description"],"parameters":function["parameters"]})) }).collect()
}
