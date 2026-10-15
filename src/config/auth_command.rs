//! `orchid auth`: add, test, and list connections; log in to Codex.
use super::{validate_connection, Connection, Settings};
use crate::client::{ClientEvent, ClientRequest, Message};
use serde_json::{json, Value};
use std::{
    fs,
    io::{self, IsTerminal, Write},
};

/// Runs the requested operation.
pub(super) fn run(settings: &Settings, args: &[String]) -> io::Result<String> {
    let name = || {
        args.get(1)
            .filter(|name| !name.starts_with('-'))
            .ok_or_else(|| invalid("auth requires a connection name"))
    };
    match args.first().map(String::as_str) {
        None | Some("list") => list(settings),
        Some("add") => add(settings, name()?, &args[2..]),
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

/// Writes connections/<name>.json for an OpenAI-compatible endpoint, from flags
/// or, on a terminal, prompts; makes it the default agent's connection if that
/// has none; then tests it.
fn add(settings: &Settings, name: &str, args: &[String]) -> io::Result<String> {
    if name == "echo" {
        return Err(invalid(
            "\"echo\" is the built-in test connection; choose another name",
        ));
    }
    let path = settings
        .root
        .join("connections")
        .join(format!("{name}.json"));
    if path.exists() {
        return Err(invalid(&format!("{} already exists", path.display())));
    }
    let interactive = io::stdin().is_terminal() && io::stderr().is_terminal();
    let ask = |flag: &str, question: &str| -> io::Result<String> {
        match flag_value(args, flag) {
            Some(value) => Ok(value),
            None if interactive => prompt(question),
            None => Ok(String::new()),
        }
    };
    let base_url = ask("--base-url", "Base URL (e.g. http://localhost:1234/v1)")?;
    let model = ask("--model", "Model")?;
    let key_env = ask(
        "--key-env",
        "Environment variable holding the API key (blank for none)",
    )?;
    let headers: serde_json::Map<String, Value> = args
        .windows(2)
        .filter(|pair| pair[0] == "--header")
        .filter_map(|pair| pair[1].split_once('='))
        .map(|(key, value)| (key.to_owned(), value.into()))
        .collect();
    let variable = |c: char| c.is_ascii_alphanumeric() || c == '_';
    if !key_env.chars().all(variable) || key_env.starts_with(|c: char| c.is_ascii_digit()) {
        // Never echo the value: it may be a pasted key.
        return Err(invalid(
            "--key-env takes the name of an environment variable, not the key",
        ));
    }
    let mut connection = json!({"interface": "openai", "base_url": base_url, "model": model});
    if !key_env.is_empty() {
        connection["api_key"] = format!("env.{key_env}").into();
    }
    if !headers.is_empty() {
        connection["headers"] = headers.into();
    }
    validate_connection(
        &serde_json::from_value::<Connection>(connection.clone()).map_err(io::Error::other)?,
    )?;
    fs::write(&path, serde_json::to_string_pretty(&connection)? + "\n")?;
    let agent_path = settings.root.join("agents/default.json");
    let mut agent: Value = super::read_json(&agent_path)?;
    let wired = agent["connection"].as_str().unwrap_or("").is_empty();
    if wired {
        agent["connection"] = name.into();
        fs::write(&agent_path, serde_json::to_string_pretty(&agent)? + "\n")?;
    }
    let tested = test(settings, name).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("saved {} but the test failed: {error}", path.display()),
        )
    })?;
    Ok(json!({"name": name, "path": path, "default": wired, "test": serde_json::from_str::<Value>(&tested)?}).to_string())
}

/// Sends one tiny request through the connection's real client.
fn test(settings: &Settings, name: &str) -> io::Result<String> {
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

/// Asks a question on stderr and reads one line from stdin.
fn prompt(question: &str) -> io::Result<String> {
    eprint!("{question}: ");
    io::stderr().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_owned())
}

/// Returns the value after `flag`, if present.
fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
}

/// Creates an invalid-input error.
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
