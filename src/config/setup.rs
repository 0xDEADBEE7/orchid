//! `orchid auth add`: guided connection setup for common providers and custom
//! endpoints. Every prompt has a flag, so scripts can run it without a terminal.
use super::prompt::{flag_value, invalid, read_line, read_secret, Prompter};
use super::{validate_connection, Connection, Settings};
use serde_json::{json, Map, Value};
use std::{
    fs,
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    time::Duration,
};

/// A provider: how its connection is written and what the user is asked.
struct Preset {
    key: &'static str,
    label: &'static str,
    base_url: &'static str,
    key_env: &'static str,
    model: &'static str,
}

/// Menu order. `codex` signs in with a ChatGPT subscription; `azure` sends the
/// key in an `api-key` header; `custom` asks for everything.
#[rustfmt::skip]
const PRESETS: &[Preset] = &[
    Preset { key: "openai", label: "OpenAI (API key)", base_url: "https://api.openai.com/v1", key_env: "OPENAI_API_KEY", model: "" },
    Preset { key: "codex", label: "ChatGPT subscription (Codex sign-in)", base_url: "https://chatgpt.com/backend-api/codex", key_env: "", model: "gpt-5.6-luna" },
    Preset { key: "anthropic", label: "Anthropic Claude (API key; Claude subscriptions can't be used here)", base_url: "https://api.anthropic.com/v1", key_env: "ANTHROPIC_API_KEY", model: "claude-opus-5-5" },
    Preset { key: "openrouter", label: "OpenRouter", base_url: "https://openrouter.ai/api/v1", key_env: "OPENROUTER_API_KEY", model: "" },
    Preset { key: "groq", label: "Groq", base_url: "https://api.groq.com/openai/v1", key_env: "GROQ_API_KEY", model: "" },
    Preset { key: "together", label: "Together", base_url: "https://api.together.ai/v1", key_env: "TOGETHER_API_KEY", model: "" },
    Preset { key: "lmstudio", label: "LM Studio (local)", base_url: "http://localhost:1234/v1", key_env: "", model: "" },
    Preset { key: "ollama", label: "Ollama (local)", base_url: "http://localhost:11434/v1", key_env: "", model: "" },
    Preset { key: "azure", label: "Azure OpenAI", base_url: "", key_env: "AZURE_OPENAI_API_KEY", model: "" },
    Preset { key: "custom", label: "Custom OpenAI-compatible endpoint (company gateway, LiteLLM, vLLM, ...)", base_url: "", key_env: "", model: "" },
];

/// Writes connections/<name>.json, optionally makes it the default agent's
/// connection, and sends a test request.
pub(super) fn add(settings: &Settings, name: Option<&str>, args: &[String]) -> io::Result<String> {
    let ui = Prompter::new(args);
    let preset = choose(&ui)?;
    let name = match name {
        Some(name) => name.to_owned(),
        None => ui.ask("--name", "Connection name", preset.key)?,
    };
    let path = settings
        .root
        .join("connections")
        .join(format!("{name}.json"));
    if name.is_empty() || name == "echo" || path.exists() {
        return Err(invalid(&format!(
            "choose another connection name ('{name}' is taken or reserved)"
        )));
    }
    let connection = match preset.key {
        "codex" => subscription(settings, &ui, preset, &name)?,
        _ => endpoint(settings, &ui, preset, &name)?,
    };
    let parsed =
        serde_json::from_value::<Connection>(connection.clone()).map_err(io::Error::other)?;
    validate_connection(&parsed)?;
    fs::write(&path, serde_json::to_string_pretty(&connection)? + "\n")?;
    let default = make_default(settings, &ui, &name)?;
    let tested = super::auth_command::test(settings, &name).map_err(|error| {
        let message = format!("saved {} but the test failed: {error}", path.display());
        io::Error::new(error.kind(), message)
    })?;
    let tested: Value = serde_json::from_str(&tested)?;
    Ok(json!({"name": name, "path": path, "default": default, "test": tested}).to_string())
}

/// Picks the provider from `--provider`, the menu, or `custom` in scripts.
fn choose(ui: &Prompter) -> io::Result<&'static Preset> {
    let wanted = match flag_value(ui.args, "--provider") {
        Some(key) => key,
        None if ui.interactive => {
            eprintln!("Provider:");
            for (number, preset) in PRESETS.iter().enumerate() {
                eprintln!("  {:>2}) {}", number + 1, preset.label);
            }
            read_line("Choose a number: ")?
        }
        None => "custom".into(),
    };
    let numbered = wanted
        .parse::<usize>()
        .ok()
        .and_then(|n| PRESETS.get(n.wrapping_sub(1)));
    numbered
        .or_else(|| PRESETS.iter().find(|preset| preset.key == wanted))
        .ok_or_else(|| invalid(&format!("unknown provider '{wanted}'")))
}

/// Signs in with a ChatGPT subscription (browser) and describes the Codex connection.
fn subscription(
    settings: &Settings,
    ui: &Prompter,
    preset: &Preset,
    name: &str,
) -> io::Result<Value> {
    let model = ui.ask("--model", "Model", preset.model)?;
    eprintln!("Signing in with your ChatGPT account...");
    crate::client::openai_codex::auth::login(&settings.root, name).map_err(io::Error::other)?;
    Ok(json!({"interface": "codex", "base_url": preset.base_url, "model": model, "auth": name}))
}

/// Asks for an OpenAI-compatible endpoint's URL, key, model and headers.
fn endpoint(settings: &Settings, ui: &Prompter, preset: &Preset, name: &str) -> io::Result<Value> {
    let hint = match preset.key {
        "azure" => "Base URL (https://<resource>.openai.azure.com/openai/v1)",
        _ => "Base URL",
    };
    let base_url = ui.ask("--base-url", hint, preset.base_url)?;
    let key = credential(settings, ui, preset, &base_url, name)?;
    let model = pick_model(ui, preset, &base_url, key.as_deref())?;
    let mut headers = headers(ui, preset)?;
    let mut connection = json!({"interface": "openai", "base_url": base_url, "model": model});
    match key {
        Some(key) if preset.key == "azure" => _ = headers.insert("api-key".into(), key.into()),
        Some(key) => connection["api_key"] = key.into(),
        None => {}
    }
    if !headers.is_empty() {
        connection["headers"] = headers.into();
    }
    Ok(connection)
}

/// Returns the key reference to store: `env.NAME`, or on a terminal (when the
/// variable is unset) a pasted key saved to a private file as `file.PATH`.
fn credential(
    settings: &Settings,
    ui: &Prompter,
    preset: &Preset,
    base_url: &str,
    name: &str,
) -> io::Result<Option<String>> {
    // A provider's key variable is only offered for that provider's own URL.
    let suggested = if base_url == preset.base_url {
        preset.key_env
    } else {
        ""
    };
    let asked = !preset.key_env.is_empty() || preset.key == "custom";
    let key_env = match asked || flag_value(ui.args, "--key-env").is_some() {
        true => ui.ask(
            "--key-env",
            "Environment variable with the API key (blank for none)",
            suggested,
        )?,
        false => String::new(),
    };
    if !key_env
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        // Never echo the value: it may be a pasted key.
        return Err(invalid(
            "--key-env takes the name of an environment variable, not the key",
        ));
    }
    let reference = Some(format!("env.{key_env}")).filter(|_| !key_env.is_empty());
    if reference.is_none() || !ui.interactive || std::env::var_os(&key_env).is_some() {
        return Ok(reference);
    }
    eprintln!("{key_env} is not set in this shell.");
    if !ui.confirm(
        "Paste the key now and store it in a private file instead",
        true,
    )? {
        eprintln!("Set it where orchid runs, e.g. `export {key_env}=...` in your shell profile.");
        return Ok(reference);
    }
    let key = read_secret("API key (input hidden): ")?;
    if key.is_empty() {
        return Err(invalid("no key entered"));
    }
    let path = settings.root.join("auth").join(format!("{name}.key"));
    fs::create_dir_all(settings.root.join("auth"))?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(&path)?;
    file.write_all(key.as_bytes())?;
    Ok(Some(format!("file.{}", path.display())))
}

/// Asks for the model, showing the endpoint's models and defaulting to the
/// provider's suggestion or the first listed model.
fn pick_model(
    ui: &Prompter,
    preset: &Preset,
    base_url: &str,
    key: Option<&str>,
) -> io::Result<String> {
    if let Some(model) = flag_value(ui.args, "--model") {
        return Ok(model);
    }
    let models = model_ids(base_url, preset.key == "azure", key);
    if ui.interactive && !models.is_empty() {
        let shown = models[..models.len().min(12)].join(", ");
        let more = models.len().saturating_sub(12);
        eprintln!(
            "Models: {shown}{}",
            if more > 0 {
                format!(" (+{more} more)")
            } else {
                String::new()
            }
        );
    }
    let first = models.first().map_or("", String::as_str);
    ui.ask(
        "--model",
        "Model",
        if preset.model.is_empty() {
            first
        } else {
            preset.model
        },
    )
}

/// Lists the endpoint's model IDs (OpenAI-compatible `GET /models`), if it answers.
fn model_ids(base_url: &str, azure: bool, key: Option<&str>) -> Vec<String> {
    let base = base_url.trim_end_matches('/');
    let url = if base.ends_with("/v1") {
        format!("{base}/models")
    } else {
        format!("{base}/v1/models")
    };
    let mut request = reqwest::blocking::Client::new()
        .get(url)
        .timeout(Duration::from_secs(10));
    if let Some(Ok(key)) = key.map(super::resolve_secret) {
        request = if azure {
            request.header("api-key", key)
        } else {
            request.bearer_auth(key)
        };
    }
    let body: Value = request
        .send()
        .and_then(|response| response.json())
        .unwrap_or_default();
    let ids = body["data"].as_array().into_iter().flatten();
    let mut ids: Vec<String> = ids
        .filter_map(|model| Some(model["id"].as_str()?.to_owned()))
        .collect();
    ids.sort();
    ids
}

/// `--header KEY=VALUE` flags, plus (for custom endpoints and Azure, on a
/// terminal) headers asked for until a blank answer.
fn headers(ui: &Prompter, preset: &Preset) -> io::Result<Map<String, Value>> {
    let pairs = ui.args.windows(2).filter(|pair| pair[0] == "--header");
    let mut headers: Map<String, Value> = pairs
        .filter_map(|pair| pair[1].split_once('='))
        .map(|(key, value)| (key.to_owned(), value.into()))
        .collect();
    if !ui.interactive || !matches!(preset.key, "custom" | "azure") {
        return Ok(headers);
    }
    let question = "Extra header KEY=VALUE (values may be env.NAME; blank to finish)";
    loop {
        let answer = ui.ask("", question, "")?;
        let Some((key, value)) = answer.split_once('=') else {
            return Ok(headers);
        };
        headers.insert(key.trim().into(), value.trim().into());
    }
}

/// Makes `name` the default agent's connection: asked on a terminal, otherwise
/// done with `--default` or when the default agent has none.
fn make_default(settings: &Settings, ui: &Prompter, name: &str) -> io::Result<bool> {
    let path = settings.root.join("agents/default.json");
    let mut agent: Value = super::read_json(&path)?;
    let unset = agent["connection"].as_str().unwrap_or("").is_empty();
    let wanted = match ui.args.iter().any(|arg| arg == "--default") {
        true => true,
        false if ui.interactive => {
            ui.confirm(&format!("Use {name} for the default agent"), unset)?
        }
        false => unset,
    };
    if wanted {
        agent["connection"] = name.into();
        fs::write(&path, serde_json::to_string_pretty(&agent)? + "\n")?;
    }
    Ok(wanted)
}
