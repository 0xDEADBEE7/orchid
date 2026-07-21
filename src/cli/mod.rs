use std::collections::BTreeMap;

pub mod output;
pub use output::{print_error, print_json};

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Help(Option<String>),
    List(Option<String>),
    Config(ConfigSubcommand),
    Auth(AuthSubcommand),
    Create {
        label: Option<String>,
        working_dir: Option<String>,
        policy: Option<String>,
        prompt: Option<String>,
        restrictions: Option<Vec<String>>,
    },
    Send {
        id: Option<String>,
        message: String,
        await_completion: bool,
        label: Option<String>,
        working_dir: Option<String>,
        policy: Option<String>,
        prompt: Option<String>,
    },
    Set {
        id: String,
        label: Option<String>,
        working_dir: Option<String>,
        restrictions: Option<Vec<String>>,
    },
    Delete(String),
    Stop(String),
    Kill(String),
    InternalRun {
        id: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigSubcommand {
    Validate,
    List,
    Show(String),
}
#[derive(Debug, Clone, PartialEq)]
pub enum AuthSubcommand {
    List,
    Validate(String),
    Login(String),
}

pub fn parse_args(args: &[String]) -> Result<(Command, BTreeMap<String, Option<String>>), String> {
    // Handle empty args: default to help
    if args.is_empty() {
        return Ok((Command::Help(None), BTreeMap::new()));
    }

    // Strip "send" prefix (for CLI usage like `orchid send list`).
    // If the first positional after "send" is not a known command,
    // default to the "send" command (so `orchid send "hi"` sends "hi").
    let (cmd_name, rest) = if args.first().map(|s| s.as_str()) == Some("send") {
        let rest = &args[1..];
        if rest.is_empty()
            || rest
                .first()
                .map(|s| s.as_str())
                .is_some_and(|s| s.starts_with("--"))
        {
            // No args or flags only: default to "send" command.
            ("send", rest)
        } else {
            // Check if the first positional is a known command.
            let known_commands = [
                "help", "list", "create", "config", "send", "set", "delete", "stop", "kill",
                "__run", "validate",
            ];
            if known_commands.contains(&rest[0].as_str()) {
                // Known command: treat it as such.
                (rest[0].as_str(), &rest[1..])
            } else {
                // Unknown: default to "send" with this as the message.
                ("send", rest)
            }
        }
    } else {
        (args[0].as_str(), &args[1..])
    };

    if cmd_name == "--help" {
        return Ok((Command::Help(None), BTreeMap::new()));
    }

    let cmd_name = cmd_name.to_string();

    // Flags that take a value argument. All others are boolean.
    // Unknown flags are rejected after command dispatch.
    const VALUE_FLAGS: &[&str] = &[
        "id",
        "label",
        "policy",
        "working-dir",
        "max-steps",
        "timeout",
        "await",
        "restriction",
        "config",
        "prompt",
    ];

    // `flags` collects all flags; for server-action, remaining flags become body params.
    let mut flags = BTreeMap::new();
    let mut positional = Vec::new();
    let mut i = 0;

    while i < rest.len() {
        let arg = &rest[i];
        if let Some(flag_suffix) = arg.strip_prefix("--") {
            if let Some(eq_pos) = flag_suffix.find('=') {
                let key = flag_suffix[..eq_pos].to_string();
                let value = flag_suffix[eq_pos + 1..].to_string();
                flags.insert(key, Some(value));
            } else {
                let key = flag_suffix.to_string();
                let takes_value = VALUE_FLAGS.contains(&key.as_str());
                if takes_value && i + 1 < rest.len() && !rest[i + 1].starts_with("--") {
                    // Boolean flags that take a value flag but should NOT consume next token.
                    if key == "await" {
                        flags.insert(key, None);
                    } else {
                        i += 1;
                        flags.insert(key, Some(rest[i].clone()));
                    }
                } else if !takes_value && i + 1 < rest.len() && !rest[i + 1].starts_with("--") {
                    // Unknown flags that have a following token are treated as value-taking
                    // (for server-action body params). For other commands, the fail-fast
                    // check catches them.
                    i += 1;
                    flags.insert(key, Some(rest[i].clone()));
                } else {
                    flags.insert(key, None);
                }
            }
        } else if !arg.starts_with("-") {
            positional.push(arg.clone());
        }
        i += 1;
    }

    if flags.contains_key("help") {
        // If cmd_name was defaulted to "send" (no explicit command given),
        // this is a top-level --help, not a subcommand --help.
        if args.first().map(|s| s.as_str()) == Some("send") {
            let rest = &args[1..];
            if rest.is_empty()
                || rest
                    .first()
                    .map(|s| s.as_str())
                    .is_some_and(|s| s.starts_with("--"))
            {
                return Ok((Command::Help(None), flags));
            }
        }
        return Ok((Command::Help(Some(cmd_name.clone())), flags));
    }

    let cmd = match cmd_name.as_str() {
        "help" => Command::Help(positional.into_iter().next()),
        "list" => {
            let resource = positional.first().cloned();
            if let Some(name) = &resource {
                if !matches!(
                    name.as_str(),
                    "sessions" | "connections" | "policies" | "prompts" | "auth"
                ) {
                    return Err(format!("unknown list resource: {}", name));
                }
            }
            Command::List(resource)
        }
        "create" => {
            let label = flags.remove("label").flatten();
            let policy = flags.remove("policy").flatten();
            let prompt = flags.remove("prompt").flatten();
            let working_dir = flags.remove("working-dir").flatten();
            let restrictions = flags
                .remove("restriction")
                .map(|v| v.map(|s| vec![s]))
                .unwrap_or_default();
            Command::Create {
                label,
                working_dir,
                policy,
                prompt,
                restrictions,
            }
        }
        "config" => {
            if positional.is_empty() {
                return Err("config requires subcommand: validate, list, or show".to_string());
            }
            match positional[0].as_str() {
                "validate" => Command::Config(ConfigSubcommand::Validate),
                "list" => Command::Config(ConfigSubcommand::List),
                "show" => {
                    let resource = positional
                        .get(1)
                        .cloned()
                        .ok_or_else(|| "config show requires <resource>".to_string())?;
                    Command::Config(ConfigSubcommand::Show(resource))
                }
                other => return Err(format!("unknown config subcommand: {}", other)),
            }
        }
        "auth" => {
            let sub = positional
                .first()
                .ok_or_else(|| "auth requires subcommand: list, validate, or login".to_string())?;
            match sub.as_str() {
                "list" => Command::Auth(AuthSubcommand::List),
                "validate" => Command::Auth(AuthSubcommand::Validate(
                    positional
                        .get(1)
                        .cloned()
                        .ok_or_else(|| "auth validate requires <name>".to_string())?,
                )),
                "login" => Command::Auth(AuthSubcommand::Login(
                    rest.get(1)
                        .cloned()
                        .ok_or_else(|| "auth login requires <name>".to_string())?,
                )),
                other => return Err(format!("unknown auth subcommand: {}", other)),
            }
        }
        "send" => {
            if positional.is_empty() {
                return Err("send requires a message".to_string());
            }
            let message = positional[0].clone();
            let id = flags.remove("id").flatten();
            let await_completion = flags.contains_key("await");
            flags.remove("await");
            let label = flags.remove("label").flatten();
            let policy = flags.remove("policy").flatten();
            let prompt = flags.remove("prompt").flatten();
            let working_dir = flags.remove("working-dir").flatten();

            // Check for unknown flags.
            if let Some(unknown) = flags
                .iter()
                .find(|(k, _v)| !VALUE_FLAGS.contains(&k.as_str()))
                .map(|(k, _)| k.as_str())
            {
                return Err(format!("unknown flag: --{}", unknown));
            }

            Command::Send {
                id,
                message,
                await_completion,
                label,
                working_dir,
                policy,
                prompt,
            }
        }
        "set" => {
            let id = flags
                .remove("id")
                .flatten()
                .ok_or_else(|| "set requires --id".to_string())?;
            let label = flags.remove("label").flatten();
            let working_dir = flags.remove("working-dir").flatten();
            let restrictions = flags
                .remove("restriction")
                .map(|v| v.map(|s| vec![s]))
                .unwrap_or_default();
            Command::Set {
                id,
                label,
                working_dir,
                restrictions,
            }
        }
        "delete" => {
            let id = positional
                .first()
                .cloned()
                .ok_or_else(|| "delete requires <id>".to_string())?;
            Command::Delete(id)
        }
        "stop" | "kill" => {
            let id = positional
                .first()
                .cloned()
                .ok_or_else(|| format!("{} requires <id>", cmd_name))?;
            if cmd_name == "stop" {
                Command::Stop(id)
            } else {
                Command::Kill(id)
            }
        }
        "__run" => {
            let id = positional
                .first()
                .cloned()
                .ok_or_else(|| "__run requires <id>".to_string())?;
            Command::InternalRun { id }
        }
        "validate" => Command::Config(ConfigSubcommand::Validate),
        _ => return Err(format!("unknown command: {}", cmd_name)),
    };

    Ok((cmd, flags))
}
