use super::HttpProvider;
use crate::model::Session;
use serde_json::{json, Value};
use std::io;

pub fn reply(provider: &HttpProvider, prompt: &str, _session: &Session) -> io::Result<String> {
    let body = body(provider, prompt, false);
    let response = provider.transport()?.request(provider, &body, false)?;
    text(response.json().map_err(io::Error::other)?)
}

fn body(provider: &HttpProvider, prompt: &str, _streaming: bool) -> Value {
    if is_codex(provider) {
        return json!({"model":provider.connection.model,"instructions":provider.system_prompt,"input":[{"role":"user","content":[{"type":"input_text","text":prompt}]}],"tools":super::tool_definitions(&provider.tools),"store":false,"stream":_streaming});
    }
    match provider.connection.interface.as_str() {
        "anthropic" => {
            json!({"model":provider.connection.model,"max_tokens":1024,"messages":[{"role":"user","content":prompt}]})
        }
        _ => {
            json!({"model":provider.connection.model,"messages":[{"role":"system","content":provider.system_prompt},{"role":"user","content":prompt}]})
        }
    }
}

fn is_codex(provider: &HttpProvider) -> bool {
    provider.connection.interface == "codex"
        || matches!(
            provider.credential,
            Some(crate::config::Credential::Codex { .. })
        )
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
