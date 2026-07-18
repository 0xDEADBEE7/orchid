use std::collections::BTreeMap;

pub mod output;
pub use output::{print_error, print_json};

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Help(Option<String>),
    List(Option<ListSubcommand>),
    Config(ConfigSubcommand),
    Create {
        label: Option<String>,
        persona: Option<String>,
        working_dir: Option<String>,
        profile: Option<String>,
        scope_exceptions: Option<Vec<String>>,
    },
    Send {
        id: Option<String>,
        message: String,
        await_completion: bool,
        profile: Option<String>,
        label: Option<String>,
        working_dir: Option<String>,
    },
    Set {
        id: String,
        label: Option<String>,
        persona: Option<String>,
        working_dir: Option<String>,
        scope_exceptions: Option<Vec<String>>,
    },
    Delete(String),
    Stop(String),
    Kill(String),
    InternalRun {
        id: String,
        profile: Option<String>,
    },
    ServerAction {
        action: String,
        profile: Option<String>,
        body_params: Vec<(String, String)>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ListSubcommand {
    Profiles,
    Personas,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigSubcommand {
    Use(String),
    Current,
    Path,
    ScopeExceptions,
}

pub fn parse_args(args: &[String]) -> Result<(Command, BTreeMap<String, Option<String>>), String> {
    // Handle empty args: default to help
    if args.is_empty() {
        return Ok((Command::Help(None), BTreeMap::new()));
    }

    // Strip "send" prefix (for CLI usage like `orchid send list`)
    let (cmd_name, rest) = if args.first().map(|s| s.as_str()) == Some("send") {
        let rest = &args[1..];
        // If no args remain or first remaining arg is a flag (starts with --),
        // default to "send" command so unknown flag errors work properly.
        let cmd_name = if rest.is_empty()
            || rest.first().map(|s| s.as_str()).is_some_and(|s| s.starts_with("--"))
        {
            "send"
        } else {
            rest[0].as_str()
        };
        let rest = if rest.is_empty()
            || rest.first().map(|s| s.as_str()).is_some_and(|s| s.starts_with("--"))
        {
            rest
        } else {
            &rest[1..]
        };
        (cmd_name, rest)
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
        "persona",
        "profile",
        "working-dir",
        "max-steps",
        "timeout",
        "await",
        "scope-exception",
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
            if rest.is_empty() || rest.first().map(|s| s.as_str()).is_some_and(|s| s.starts_with("--")) {
                return Ok((Command::Help(None), flags));
            }
        }
        return Ok((Command::Help(Some(cmd_name.clone())), flags));
    }

    let cmd = match cmd_name.as_str() {
        "help" => Command::Help(positional.into_iter().next()),
        "list" => {
            let sub = match positional.first().map(|s| s.as_str()) {
                Some("profiles") => Some(ListSubcommand::Profiles),
                Some("personas") => Some(ListSubcommand::Personas),
                Some(other) => return Err(format!("unknown list subcommand: {}", other)),
                None => None,
            };
            Command::List(sub)
        }
        "create" => {
            let label = flags.remove("label").flatten();
            let persona = flags.remove("persona").flatten();
            let working_dir = flags.remove("working-dir").flatten();
            let profile = flags.remove("profile").flatten();
            let scope_exceptions = flags
                .remove("scope-exception")
                .map(|v| v.map(|s| vec![s]))
                .unwrap_or_default();
            Command::Create {
                label,
                persona,
                working_dir,
                profile,
                scope_exceptions,
            }
        }
        "config" => {
            if positional.is_empty() {
                return Err("config requires subcommand: use, current, or path".to_string());
            }
            let sub = &positional[0];
            match sub.as_str() {
                "use" => {
                    if positional.len() < 2 {
                        return Err("config use requires <profile> argument".to_string());
                    }
                    Command::Config(ConfigSubcommand::Use(positional[1].clone()))
                }
                "current" => Command::Config(ConfigSubcommand::Current),
                "path" => Command::Config(ConfigSubcommand::Path),
                "scope-exceptions" => Command::Config(ConfigSubcommand::ScopeExceptions),
                _ => return Err(format!("unknown config subcommand: {}", sub)),
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
            let profile = flags.remove("profile").flatten();
            let label = flags.remove("label").flatten();
            let working_dir = flags.remove("working-dir").flatten();

            // Check for unknown flags.
            if let Some(unknown) = flags.iter().find(|(k, _v)| {
                !VALUE_FLAGS.contains(&k.as_str())
            }).map(|(k, _)| k.as_str()) {
                return Err(format!("unknown flag: --{}", unknown));
            }

            Command::Send {
                id,
                message,
                await_completion,
                profile,
                label,
                working_dir,
            }
        }
        "set" => {
            let id = flags
                .remove("id")
                .flatten()
                .ok_or_else(|| "set requires --id".to_string())?;
            let label = flags.remove("label").flatten();
            let persona = flags.remove("persona").flatten();
            let working_dir = flags.remove("working-dir").flatten();
            let scope_exceptions = flags
                .remove("scope-exception")
                .map(|v| v.map(|s| vec![s]))
                .unwrap_or_default();
            if flags.remove("profile").is_some() {
                return Err("--profile is not supported on set; use `orchid config use <name>` to switch the active profile".to_string());
            }

            Command::Set {
                id,
                label,
                persona,
                working_dir,
                scope_exceptions,
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
            let profile = flags.remove("profile").flatten();
            Command::InternalRun { id, profile }
        }
        "server-action" => {
            let action = positional
                .first()
                .cloned()
                .ok_or_else(|| "server-action requires <action>".to_string())?;
            let profile = flags.remove("profile").flatten();
            // Remaining flags in `flags` are body params for the action.
            let mut body_params = Vec::new();
            for (k, v) in std::mem::take(&mut flags).into_iter() {
                if let Some(val) = v {
                    body_params.push((k, val));
                }
            }
            return Ok((Command::ServerAction {
                action,
                profile,
                body_params,
            }, flags));
        }
        _ => return Err(format!("unknown command: {}", cmd_name)),
    };

    Ok((cmd, flags))
}
