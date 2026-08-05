use super::{read_json, Settings};
use serde_json::Value;
use std::{fs, io};

pub(super) fn run(settings: &Settings, args: &[String]) -> io::Result<String> {
    match args.first().map(String::as_str) {
        None | Some("list") => list(settings),
        Some("validate") => validate(settings, args.get(1)),
        Some("login") => login(settings, args.get(1)),
        Some(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unknown auth command",
        )),
    }
}

fn list(settings: &Settings) -> io::Result<String> {
    let names = fs::read_dir(settings.root.join("auth"))?
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect::<Vec<_>>();
    serde_json::to_string(&serde_json::json!({"auth":names})).map_err(io::Error::other)
}

fn validate(settings: &Settings, name: Option<&String>) -> io::Result<String> {
    let name = name.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "auth validate requires a name")
    })?;
    if settings
        .root
        .join("auth/tokens")
        .join(format!("{name}.json"))
        .exists()
    {
        crate::client::openai_codex::auth::access_token(&settings.root, name).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "Codex credential unavailable")
        })?;
        return Ok(serde_json::json!({"valid":true,"type":"openai_codex_oauth"}).to_string());
    }
    let _: Value = read_json(&settings.root.join("auth").join(format!("{name}.json")))?;
    Ok(serde_json::json!({"valid":true}).to_string())
}

fn login(settings: &Settings, name: Option<&String>) -> io::Result<String> {
    let name = name
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "auth login requires a name"))?;
    if name == "codex"
        || settings
            .root
            .join("auth/tokens")
            .join(format!("{name}.json"))
            .exists()
    {
        return crate::client::openai_codex::auth::login(&settings.root, name)
            .map_err(io::Error::other)
            .map(|value| value.to_string());
    }
    let key = std::env::var("ORCHID_API_KEY")
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "ORCHID_API_KEY is required"))?;
    let body = serde_json::to_vec(&serde_json::json!({"type":"api_key","value":key}))
        .map_err(io::Error::other)?;
    fs::create_dir_all(settings.root.join("auth"))?;
    fs::write(
        settings.root.join("auth").join(format!("{name}.json")),
        body,
    )?;
    Ok(serde_json::json!({"name":name}).to_string())
}
