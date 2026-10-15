//! `orchid auth`: add (guided, see setup.rs), test, and list connections; log
//! in to Codex.
use super::Settings;
use super::{prompt::invalid, setup::add};
use crate::client::{ClientEvent, ClientRequest, Message};
use serde_json::json;
use std::{fs, io};

/// Runs the requested operation.
pub(super) fn run(settings: &Settings, args: &[String]) -> io::Result<String> {
    let given = args.get(1).filter(|name| !name.starts_with('-'));
    let name = || given.ok_or_else(|| invalid("auth requires a connection name"));
    match args.first().map(String::as_str) {
        None | Some("list") => list(settings),
        Some("add") => add(
            settings,
            given.map(String::as_str),
            &args[1 + usize::from(given.is_some())..],
        ),
        Some("test") => test(settings, name()?),
        Some("login") => crate::client::openai_codex::auth::login(&settings.root, name()?)
            .map_err(io::Error::other)
            .map(|value| value.to_string()),
        Some(other) => Err(invalid(&format!(
            "unknown auth command: {other} (use list, add, test, login)"
        ))),
    }
}

/// Lists connections and Codex logins.
fn list(settings: &Settings) -> io::Result<String> {
    let names = |dir: &str| -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(settings.root.join(dir))
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
            .filter_map(|file| file.strip_suffix(".json").map(str::to_owned))
            .collect();
        names.sort();
        names
    };
    Ok(json!({"connections": names("connections"), "logins": names("auth/tokens")}).to_string())
}

/// Sends one tiny request through the connection's real client.
pub(super) fn test(settings: &Settings, name: &str) -> io::Result<String> {
    let context =
        |error: &dyn std::fmt::Display| io::Error::other(format!("connection {name}: {error}"));
    let resolved = settings.resolve_connection(name).map_err(|e| context(&e))?;
    let client = crate::client::client_for(resolved.clone()).map_err(|e| context(&e))?;
    let request = ClientRequest {
        model: resolved.connection.model,
        system_prompt: String::new(),
        messages: vec![Message {
            role: "user".into(),
            content: "Reply with OK.".into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }],
        tools: Vec::new(),
        params: resolved.params.into_iter().collect(),
    };
    let events = client.stream(request).map_err(|e| context(&e))?;
    let reply: String = events
        .iter()
        .filter_map(|event| match event {
            ClientEvent::TextDelta(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    Ok(json!({"name": name, "ok": true, "reply": reply}).to_string())
}
