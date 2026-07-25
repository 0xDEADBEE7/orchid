use super::{ClientError, ClientErrorKind};
use crate::config::Credential;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::Path,
};

const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const AUTHORIZE_URL: &str = "https://auth.openai.com/oauth/authorize";
const REDIRECT_URI: &str = "http://localhost:1455/auth/callback";
const DEFAULT_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
    pub account_id: String,
}

fn token_path(root: &Path, name: &str) -> std::path::PathBuf {
    root.join("auth/tokens").join(format!("{name}.json"))
}

fn account_id(value: &serde_json::Value) -> Option<String> {
    value["account_id"].as_str().map(str::to_owned).or_else(|| {
        let token = value["id_token"].as_str()?;
        let payload = token.split('.').nth(1)?;
        let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
        let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
        claims["https://api.openai.com/auth"]["chatgpt_account_id"]
            .as_str()
            .map(str::to_owned)
    })
}

pub fn load(root: &Path, name: &str) -> Result<CodexTokens, String> {
    serde_json::from_slice(
        &std::fs::read(token_path(root, name))
            .map_err(|_| format!("Codex OAuth login required for {name}"))?,
    )
    .map_err(|_| "invalid Codex OAuth token file".into())
}

pub fn save(root: &Path, name: &str, tokens: &CodexTokens) -> Result<(), String> {
    let path = token_path(root, name);
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(
        &path,
        serde_json::to_vec(tokens).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn access_token(root: &Path, name: &str) -> Result<CodexTokens, String> {
    let mut tokens = load(root, name)?;
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
        .map_err(|e| format!("Codex OAuth refresh failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Codex OAuth refresh failed (HTTP {})",
            response.status()
        ));
    }
    let value: serde_json::Value = response
        .json()
        .map_err(|_| "invalid OAuth refresh response")?;
    tokens.access_token = value["access_token"]
        .as_str()
        .ok_or("OAuth refresh response missing access token")?
        .into();
    if let Some(refresh) = value["refresh_token"].as_str() {
        tokens.refresh_token = refresh.into();
    }
    tokens.expires_at =
        chrono::Utc::now().timestamp() + value["expires_in"].as_i64().unwrap_or(3600);
    save(root, name, &tokens)?;
    Ok(tokens)
}

pub fn login(root: &Path, name: &str) -> Result<serde_json::Value, String> {
    let mut random = [0u8; 32];
    getrandom::getrandom(&mut random).map_err(|e| e.to_string())?;
    let state = URL_SAFE_NO_PAD.encode(random);
    getrandom::getrandom(&mut random).map_err(|e| e.to_string())?;
    let verifier = URL_SAFE_NO_PAD.encode(random);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let client_id =
        std::env::var("ORCHID_CODEX_CLIENT_ID").unwrap_or_else(|_| DEFAULT_CLIENT_ID.into());
    let url = format!("{AUTHORIZE_URL}?response_type=code&client_id={client_id}&redirect_uri={REDIRECT_URI}&scope=openid%20profile%20email%20offline_access&state={state}&code_challenge={challenge}&code_challenge_method=S256&id_token_add_organizations=true&codex_cli_simplified_flow=true");
    let listener =
        TcpListener::bind("127.0.0.1:1455").map_err(|e| format!("callback bind failed: {e}"))?;
    let _ = std::process::Command::new("open").arg(&url).status();
    let (mut stream, _) = listener.accept().map_err(|e| e.to_string())?;
    let mut buf = [0u8; 8192];
    let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
    let request = String::from_utf8_lossy(&buf[..n]);
    let target = request
        .split_whitespace()
        .nth(1)
        .ok_or("invalid OAuth callback")?;
    let query = target.split('?').nth(1).unwrap_or("");
    let mut code = None;
    let mut returned_state = None;
    for part in query.split('&') {
        let mut pair = part.splitn(2, '=');
        match pair.next().unwrap_or("") {
            "code" => code = pair.next().map(str::to_owned),
            "state" => returned_state = pair.next().map(str::to_owned),
            _ => {}
        }
    }
    if returned_state.as_deref() != Some(&state) {
        let _ = stream.write_all(
            b"HTTP/1.1 400 Bad Request\r\nContent-Length: 25\r\n\r\nOAuth state validation failed",
        );
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
        let message = format!(
            "Codex OAuth token exchange failed (HTTP {})",
            response.status()
        );
        let body = format!("OAuth login failed: {message}");
        let _ = stream.write_all(
            format!(
                "HTTP/1.1 400 Bad Request\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            )
            .as_bytes(),
        );
        return Err(message);
    }
    let value: serde_json::Value = response
        .json()
        .map_err(|_| "invalid OAuth token response")?;
    let access = value["access_token"]
        .as_str()
        .ok_or("OAuth response missing access token")?;
    let refresh = value["refresh_token"]
        .as_str()
        .ok_or("OAuth response missing refresh token")?;
    let account = match account_id(&value) {
        Some(account) => account,
        None => {
            let body = "OAuth login failed: response missing account ID";
            let _ = stream.write_all(
                format!(
                    "HTTP/1.1 400 Bad Request\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                )
                .as_bytes(),
            );
            return Err("OAuth response missing account ID".into());
        }
    };
    save(
        root,
        name,
        &CodexTokens {
            access_token: access.into(),
            refresh_token: refresh.into(),
            expires_at: chrono::Utc::now().timestamp()
                + value["expires_in"].as_i64().unwrap_or(3600),
            account_id: account,
        },
    )?;
    let _ = stream
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 25\r\n\r\nOrchid login successful.\n");
    Ok(serde_json::json!({"status":"ok","name":name}))
}

pub struct CodexAuth;

impl CodexAuth {
    pub fn present(credential: &Credential) -> Result<(&str, &str), ClientError> {
        match credential {
            Credential::Codex {
                access_token,
                account_id,
            } if !access_token.is_empty() => Ok((access_token, account_id)),
            _ => Err(ClientError::new(
                ClientErrorKind::Authentication,
                "Codex OAuth credential required",
            )),
        }
    }
}
