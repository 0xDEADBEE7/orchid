use std::collections::BTreeMap;

use super::{auth, await_command, config, create, delete, get, internal_run, lifecycle, set};

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
    Await {
        ids: Vec<String>,
        timeout: f64,
        interval: f64,
    },
    Get {
        id: String,
        conversation: bool,
        last_message: bool,
        metadata: bool,
        state: bool,
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
    Use(String),
}
#[derive(Debug, Clone, PartialEq)]
pub enum AuthSubcommand {
    List,
    Validate(String),
    Login(String),
}

const VALUE_FLAGS: &[&str] = &[
    "id",
    "label",
    "policy",
    "working-dir",
    "max-steps",
    "timeout",
    "interval",
    "await",
    "restriction",
    "config",
    "prompt",
];

struct ParsedInput<'a> {
    name: String,
    rest: &'a [String],
    top_level_help: bool,
}

fn detect_command(args: &[String]) -> ParsedInput<'_> {
    if args.first().map(String::as_str) != Some("send") {
        return ParsedInput {
            name: args[0].clone(),
            rest: &args[1..],
            top_level_help: false,
        };
    }
    let rest = &args[1..];
    if rest.is_empty() || rest[0].starts_with("--") {
        return ParsedInput {
            name: "send".to_string(),
            rest,
            top_level_help: true,
        };
    }
    let known = [
        "help", "list", "create", "config", "send", "await", "get", "set", "delete", "stop",
        "kill", "__run", "validate",
    ];
    if known.contains(&rest[0].as_str()) {
        ParsedInput {
            name: rest[0].clone(),
            rest: &rest[1..],
            top_level_help: false,
        }
    } else {
        ParsedInput {
            name: "send".to_string(),
            rest,
            top_level_help: false,
        }
    }
}

fn tokenize_flags(rest: &[String]) -> (BTreeMap<String, Option<String>>, Vec<String>) {
    let mut flags = BTreeMap::new();
    let mut positional = Vec::new();
    let mut index = 0;
    while index < rest.len() {
        let arg = &rest[index];
        if let Some(suffix) = arg.strip_prefix("--") {
            let (key, inline) = suffix
                .split_once('=')
                .map_or((suffix, None), |(key, value)| (key, Some(value)));
            if let Some(value) = inline {
                flags.insert(key.to_string(), Some(value.to_string()));
            } else {
                let takes_value = VALUE_FLAGS.contains(&key) && key != "await";
                let has_value = index + 1 < rest.len() && !rest[index + 1].starts_with("--");
                if (takes_value || !VALUE_FLAGS.contains(&key)) && has_value {
                    index += 1;
                    flags.insert(key.to_string(), Some(rest[index].clone()));
                } else {
                    flags.insert(key.to_string(), None);
                }
            }
        } else if !arg.starts_with('-') {
            positional.push(arg.clone());
        }
        index += 1;
    }
    (flags, positional)
}

pub(crate) fn parse(
    filtered_args: &[String],
    global_flags: BTreeMap<String, Option<String>>,
) -> Result<(Command, BTreeMap<String, Option<String>>), String> {
    let args = filtered_args;
    if args.is_empty() {
        return Ok((Command::Help(None), BTreeMap::new()));
    }
    let input = detect_command(args);
    let cmd_name = input.name;
    let rest = input.rest;
    if cmd_name == "--help" {
        return Ok((Command::Help(None), BTreeMap::new()));
    }

    let (mut flags, positional) = tokenize_flags(rest);

    if flags.contains_key("help") {
        if input.top_level_help {
            return Ok((Command::Help(None), flags));
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
        "create" => create::parse(&mut flags),
        "config" => config::parse(&positional)?,
        "auth" => auth::parse(&positional, rest)?,
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
        "get" => get::parse(&mut flags, &positional)?,
        "await" => await_command::parse(&mut flags, positional)?,
        "set" => set::parse(&mut flags)?,
        "delete" => delete::parse(&positional)?,
        "stop" | "kill" => lifecycle::parse(&positional, &cmd_name)?,
        "__run" => internal_run::parse(&positional)?,
        "validate" => Command::Config(ConfigSubcommand::Validate),
        _ => return Err(format!("unknown command: {}", cmd_name)),
    };

    for (key, value) in global_flags {
        flags.insert(key, value);
    }
    Ok((cmd, flags))
}
