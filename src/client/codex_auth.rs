//! Codex OAuth token persistence and refresh.
use crate::config::ConfigDir;
use serde::{Deserialize, Serialize};

const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
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

pub fn save(dir: &ConfigDir, name: &str, tokens: &CodexTokens) -> Result<(), String> {
    let path = token_path(dir, name);
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&path, serde_json::to_vec(tokens).unwrap()).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn load(dir: &ConfigDir, name: &str) -> Result<CodexTokens, String> {
    let path = token_path(dir, name);
    serde_json::from_slice(
        &std::fs::read(&path).map_err(|_| format!("Codex OAuth login required for {}", name))?,
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
    let value: serde_json::Value = response
        .json()
        .map_err(|_| "invalid OAuth refresh response")?;
    tokens.access_token = value["access_token"]
        .as_str()
        .ok_or("OAuth refresh response missing access token")?
        .into();
    if let Some(refresh_token) = value["refresh_token"].as_str() {
        tokens.refresh_token = refresh_token.into();
    }
    tokens.expires_at =
        chrono::Utc::now().timestamp() + value["expires_in"].as_i64().unwrap_or(3600);
    save(dir, name, &tokens)?;
    Ok(tokens)
}

pub fn validate_token(dir: &ConfigDir, name: &str) -> Result<serde_json::Value, String> {
    let tokens = load(dir, name)?;
    Ok(serde_json::json!({
        "status": "ok",
        "type": "openai_codex_oauth",
        "credential_present": !tokens.access_token.is_empty()
    }))
}
