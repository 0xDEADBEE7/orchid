//! OpenAI Codex (ChatGPT account) OAuth transport and token storage.
//! This is deliberately separate from the public OpenAI API client.
use crate::config::ConfigDir;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;

const AUTHORIZE_URL: &str = "https://auth.openai.com/oauth/authorize";
const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const REDIRECT_URI: &str = "http://localhost:1455/auth/callback";
const CODEX_ENDPOINT: &str = "https://chatgpt.com/backend-api/codex/responses";
const DEFAULT_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
    pub account_id: String,
}

fn token_path(dir: &ConfigDir, name: &str) -> std::path::PathBuf {
    dir.auth_path()
        .join("tokens")
        .join(format!("{}.json", name))
}
fn b64url(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}
fn jwt_claims(token: &str) -> Option<serde_json::Value> {
    let p = token.split('.').nth(1)?;
    serde_json::from_slice(&URL_SAFE_NO_PAD.decode(p).ok()?).ok()
}
fn random_hex(n: usize) -> String {
    let mut b = vec![0u8; n];
    getrandom::getrandom(&mut b).expect("OS randomness unavailable");
    hex::encode(b)
}

fn responses_content_type(role: &str) -> &'static str {
    if role == "assistant" {
        "output_text"
    } else {
        "input_text"
    }
}

fn codex_tool_definitions() -> Vec<serde_json::Value> {
    crate::tools::tool_definitions()
        .into_iter()
        .map(|tool| {
            serde_json::json!({
                "type": "function",
                "name": tool["name"],
                "description": tool["description"],
                "parameters": tool["input_schema"]
            })
        })
        .collect()
}

fn codex_input_items(messages: &[crate::types::Message]) -> Vec<serde_json::Value> {
    messages.iter().flat_map(|message| {
        if let Some(calls) = &message.tool_calls {
            return calls.iter().map(|call| serde_json::json!({
                "type": "function_call",
                "call_id": call.id,
                "name": call.name,
                "arguments": call.input.to_string()
            })).collect::<Vec<_>>();
        }
        if let Some(result) = &message.tool_result {
            return vec![serde_json::json!({
                "type": "function_call_output",
                "call_id": result.call_id,
                "output": result.content.to_string()
            })];
        }
        vec![serde_json::json!({
            "role": message.role,
            "content": [{
                "type": responses_content_type(&message.role),
                "text": message.content
            }]
        })]
    }).collect()
}

fn parse_codex_output(raw: &str, model: &str) -> Result<crate::provider::Response, crate::provider::ProviderError> {
    let values = if let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) {
        vec![v]
    } else {
        raw.lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .filter(|data| *data != "[DONE]")
            .filter_map(|data| serde_json::from_str::<serde_json::Value>(data).ok())
            .collect()
    };
    let mut delta_text = String::new();
    let mut completed_text = String::new();
    let mut tool_calls = Vec::new();
    for value in values {
        let mut items = value["output"].as_array().cloned().unwrap_or_default();
        if let Some(item) = value.get("item") {
            items.push(item.clone());
        }
        for item in items {
                if item["type"] == "function_call" {
                    let Some(arguments) = item["arguments"].as_str() else { continue };
                    let Ok(input) = serde_json::from_str(arguments) else { continue };
                    tool_calls.push(crate::types::ToolCall {
                        id: item["call_id"].as_str().or_else(|| item["id"].as_str()).unwrap_or_default().to_string(),
                        name: item["name"].as_str().unwrap_or_default().to_string(),
                        input,
                    });
                }
                if let Some(t) = item["content"][0]["text"].as_str() { completed_text.push_str(t); }
        }
        if value["type"].as_str().is_some_and(|kind| {
            kind == "response.output_text.delta" || kind == "output_text.delta"
        }) {
            if let Some(t) = value["delta"].as_str() { delta_text.push_str(t); }
        }
        if value["type"].as_str().is_none() || value["type"].as_str().is_some_and(|kind| {
            kind == "response.completed" || kind == "response.output_text.done"
        }) {
            if let Some(t) = value["output_text"].as_str() { completed_text.push_str(t); }
        }
    }
    let text = if !delta_text.is_empty() { delta_text } else { completed_text };
    if text.is_empty() && tool_calls.is_empty() {
        return Err(crate::provider::ProviderError::InvalidResponse("Codex response contained no text output or tool call".into()));
    }
    Ok(crate::provider::Response { message: (!text.is_empty()).then_some(text), reasoning: None, tool_calls: (!tool_calls.is_empty()).then_some(tool_calls), usage: None, model: Some(model.to_string()) })
}
fn save(dir: &ConfigDir, name: &str, tokens: &CodexTokens) -> Result<(), String> {
    let p = token_path(dir, name);
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&p, serde_json::to_vec(tokens).unwrap()).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn load(dir: &ConfigDir, name: &str) -> Result<CodexTokens, String> {
    let p = token_path(dir, name);
    serde_json::from_slice(
        &std::fs::read(&p).map_err(|_| format!("Codex OAuth login required for {}", name))?,
    )
    .map_err(|_| "invalid Codex OAuth token file".into())
}

pub fn access_token(dir: &ConfigDir, name: &str) -> Result<CodexTokens, String> {
    let mut tokens = load(dir, name)?;
    if tokens.expires_at > chrono::Utc::now().timestamp() + 60 {
        return Ok(tokens);
    }
    let client_id =
        std::env::var("ORCHID_CODEX_CLIENT_ID").unwrap_or_else(|_| DEFAULT_CLIENT_ID.into());
    let response = reqwest::blocking::Client::new()
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", tokens.refresh_token.as_str()),
            ("client_id", client_id.as_str()),
        ])
        .send()
        .map_err(|e| format!("Codex OAuth refresh failed: {}", e))?;
    if !response.status().is_success() {
        return Err(format!(
            "Codex OAuth refresh failed (HTTP {})",
            response.status()
        ));
    }
    let v: serde_json::Value = response
        .json()
        .map_err(|_| "invalid OAuth refresh response")?;
    tokens.access_token = v["access_token"]
        .as_str()
        .ok_or("OAuth refresh response missing access token")?
        .into();
    if let Some(r) = v["refresh_token"].as_str() {
        tokens.refresh_token = r.into();
    }
    tokens.expires_at = chrono::Utc::now().timestamp() + v["expires_in"].as_i64().unwrap_or(3600);
    save(dir, name, &tokens)?;
    Ok(tokens)
}

pub fn model_allowed(model: &str) -> bool {
    matches!(model, "gpt-5" | "gpt-5-codex" | "codex-mini-latest" | "gpt-5.6-luna")
}

pub fn validate_token(dir: &ConfigDir, name: &str) -> Result<serde_json::Value, String> {
    let t = load(dir, name)?;
    Ok(
        serde_json::json!({"status":"ok","type":"openai_codex_oauth","credential_present":!t.access_token.is_empty()}),
    )
}

pub fn login(dir_path: &Path, name: &str) -> Result<serde_json::Value, String> {
    let dir = ConfigDir::new(dir_path);
    let state = random_hex(32);
    let verifier = random_hex(32);
    let challenge = b64url(&Sha256::digest(verifier.as_bytes()));
    let client_id =
        std::env::var("ORCHID_CODEX_CLIENT_ID").unwrap_or_else(|_| DEFAULT_CLIENT_ID.into());
    let url=format!("{}?response_type=code&client_id={}&redirect_uri={}&scope=openid%20profile%20email%20offline_access&state={}&code_challenge={}&code_challenge_method=S256&id_token_add_organizations=true&codex_cli_simplified_flow=true", AUTHORIZE_URL, client_id, REDIRECT_URI, state, challenge);
    let listener =
        TcpListener::bind("127.0.0.1:1455").map_err(|e| format!("callback bind failed: {}", e))?;
    let _ = std::process::Command::new("open").arg(&url).status();
    let (mut stream, _) = listener.accept().map_err(|e| e.to_string())?;
    let mut buf = [0; 8192];
    let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
    let request = String::from_utf8_lossy(&buf[..n]);
    let target = request
        .split_whitespace()
        .nth(1)
        .ok_or("invalid OAuth callback")?;
    let query = target.split('?').nth(1).unwrap_or("");
    let mut code = None;
    let mut returned_state = None;
    for p in query.split('&') {
        let mut i = p.splitn(2, '=');
        match i.next().unwrap_or("") {
            "code" => code = i.next().map(str::to_string),
            "state" => returned_state = i.next().map(str::to_string),
            _ => {}
        }
    }
    if returned_state.as_deref() != Some(state.as_str()) {
        return Err("OAuth state validation failed".into());
    }
    let code = code.ok_or("OAuth callback did not contain an authorization code")?;
    let response = reqwest::blocking::Client::new()
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id.as_str()),
            ("code", code.as_str()),
            ("redirect_uri", REDIRECT_URI),
            ("code_verifier", verifier.as_str()),
        ])
        .send()
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "Codex OAuth token exchange failed (HTTP {})",
            response.status()
        ));
    }
    let v: serde_json::Value = response
        .json()
        .map_err(|_| "invalid OAuth token response")?;
    let access = v["access_token"]
        .as_str()
        .ok_or("OAuth response missing access token")?
        .to_string();
    let refresh = v["refresh_token"]
        .as_str()
        .ok_or("OAuth response missing refresh token")?
        .to_string();
    let account = v["account_id"]
        .as_str()
        .map(str::to_string)
        .or_else(|| {
            jwt_claims(v["id_token"].as_str()?).and_then(|c| {
                c["https://api.openai.com/auth"]["chatgpt_account_id"]
                    .as_str()
                    .map(str::to_string)
            })
        })
        .unwrap_or_default();
    if account.is_empty() {
        return Err("OAuth response missing account ID".into());
    }
    save(
        &dir,
        name,
        &CodexTokens {
            access_token: access,
            refresh_token: refresh,
            expires_at: chrono::Utc::now().timestamp() + v["expires_in"].as_i64().unwrap_or(3600),
            account_id: account,
        },
    )?;
    let _ = stream
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 25\r\n\r\nOrchid login successful.\n");
    Ok(serde_json::json!({"status":"ok","name":name}))
}

pub fn endpoint() -> &'static str {
    CODEX_ENDPOINT
}

pub struct CodexClient {
    connection: crate::config::Connection,
}
impl CodexClient {
    pub fn from_connection(
        connection: &crate::config::Connection,
    ) -> Result<Self, crate::provider::ProviderError> {
        if !model_allowed(&connection.model) {
            return Err(crate::provider::ProviderError::AuthError(
                "model is not allowed for OpenAI Codex OAuth".into(),
            ));
        }
        if connection.auth_profile.as_ref().map(|p| p.kind.as_str()) != Some("openai_codex_oauth") {
            return Err(crate::provider::ProviderError::AuthError(
                "Codex provider requires openai_codex_oauth".into(),
            ));
        }
        if connection.auth_storage.is_none() {
            return Err(crate::provider::ProviderError::AuthError(
                "Codex OAuth storage is unavailable".into(),
            ));
        }
        Ok(Self {
            connection: connection.clone(),
        })
    }
    fn request(
        &self,
        system: String,
        messages: Vec<crate::types::Message>,
    ) -> Result<crate::provider::Response, crate::provider::ProviderError> {
        let dir = ConfigDir::new(
            self.connection
                .auth_storage
                .as_ref()
                .unwrap()
                .parent()
                .unwrap(),
        );
        let tokens = access_token(
            &dir,
            self.connection.auth.as_deref().unwrap_or("openai-codex"),
        )
        .map_err(crate::provider::ProviderError::AuthError)?;
        let mut input = Vec::new();
        if !system.is_empty() {
            input.push(serde_json::json!({"role":"developer","content":[{"type":"input_text","text":system}]}));
        }
        input.extend(codex_input_items(&messages));
        let body = serde_json::json!({"model":self.connection.model,"instructions":"You are Orchid, a helpful coding assistant.","input":input,"tools":codex_tool_definitions(),"store":false,"stream":true});
        let session_id = uuid::Uuid::new_v4().to_string();
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(15))
            .timeout(std::time::Duration::from_secs(120))
            .user_agent("codex_cli_rs/0.144.4 (orchid)")
            .build()
            .map_err(|e| crate::provider::ProviderError::Network(e.to_string()))?;
        let response = client
            .post(CODEX_ENDPOINT)
            .header("Authorization", format!("Bearer {}", tokens.access_token))
            .header("ChatGPT-Account-ID", tokens.account_id)
            .header("originator", "codex_cli_rs")
            .header("openai-beta", "responses=experimental")
            .header("Version", "0.144.4")
            .header("Session_Id", session_id)
            .header("Accept", "text/event-stream")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .map_err(|e| {
                crate::provider::ProviderError::Network(format!("Codex request failed: {}", e))
            })?;
        if response.status().as_u16() == 401 {
            return Err(crate::provider::ProviderError::AuthError(
                "Codex access token rejected".into(),
            ));
        }
        if !response.status().is_success() {
            let status = response.status();
            let mut detail = response.text().unwrap_or_default();
            if detail.len() > 600 { detail.truncate(600); detail.push_str("..."); }
            detail = detail.replace(&tokens.access_token, "[REDACTED]").replace(&tokens.refresh_token, "[REDACTED]");
            return Err(crate::provider::ProviderError::InvalidResponse(format!(
                "Codex backend HTTP {}: {}",
                status, detail
            )));
        }
        let raw = response.text().map_err(|e| {
            crate::provider::ProviderError::Network(format!("Codex response read failed: {}", e))
        })?;
        parse_codex_output(&raw, &self.connection.model)
    }
}

#[cfg(test)]
mod tests {
    use super::{codex_input_items, parse_codex_output, responses_content_type};
    use crate::types::{Message, ToolCall, ToolResult};
    use serde_json::json;

    #[test]
    fn codex_responses_use_role_appropriate_content_types() {
        assert_eq!(responses_content_type("user"), "input_text");
        assert_eq!(responses_content_type("developer"), "input_text");
        assert_eq!(responses_content_type("assistant"), "output_text");
    }

    #[test]
    fn parses_streamed_function_call_completion() {
        let raw = "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"bash\",\"arguments\":\"{\\\"cmd\\\":\\\"ls -la\\\"}\"}}\n";
        let response = parse_codex_output(raw, "gpt-5.6-luna").unwrap();
        let calls = response.tool_calls.unwrap();
        assert_eq!(calls[0].name, "bash");
        assert_eq!(calls[0].input["cmd"], "ls -la");
    }

    #[test]
    fn serializes_codex_tool_turns_as_responses_items() {
        let messages = vec![
            Message { role: "user".into(), content: "run it".into(), tool_calls: None, tool_result: None },
            Message { role: "assistant".into(), content: String::new(), tool_calls: Some(vec![ToolCall { id: "call_1".into(), name: "bash".into(), input: json!({"cmd":"ls -la"}) }]), tool_result: None },
            Message { role: "user".into(), content: String::new(), tool_calls: None, tool_result: Some(ToolResult { call_id: "call_1".into(), content: json!("file.txt") }) },
        ];
        let items = codex_input_items(&messages);
        assert_eq!(items[1]["type"], "function_call");
        assert_eq!(items[1]["arguments"], "{\"cmd\":\"ls -la\"}");
        assert_eq!(items[2]["type"], "function_call_output");
        assert_eq!(items[2]["call_id"], "call_1");
    }

    #[test]
    fn does_not_append_completed_text_after_deltas() {
        let raw = concat!(
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hello\"}\n",
            "data: {\"type\":\"response.output_text.done\",\"output_text\":\"Hello\"}\n",
        );
        let response = parse_codex_output(raw, "gpt-5.6-luna").unwrap();
        assert_eq!(response.message.as_deref(), Some("Hello"));
    }
}
impl crate::provider::Provider for CodexClient {
    fn send(
        &self,
        system: String,
        messages: Vec<crate::types::Message>,
    ) -> Result<crate::provider::Response, crate::provider::ProviderError> {
        self.request(system, messages)
    }
    fn send_streaming(
        &self,
        system: String,
        messages: Vec<crate::types::Message>,
    ) -> Result<
        Box<
            dyn Iterator<
                Item = Result<crate::provider::StreamEvent, crate::provider::ProviderError>,
            >,
        >,
        crate::provider::ProviderError,
    > {
        let r = self.request(system, messages)?;
        Ok(Box::new(std::iter::once(Ok(
            crate::provider::StreamEvent::Complete(r),
        ))))
    }
}
