use std::collections::BTreeMap;

use super::{
    auth, await_command, config, create, delete, get, help, internal_run, lifecycle, list, send,
    set,
};

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

pub(super) const VALUE_FLAGS: &[&str] = &[
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

pub(super) struct ParsedInput<'a> {
    pub(super) name: String,
    pub(super) rest: &'a [String],
    pub(super) top_level_help: bool,
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
    let cmd_name = input.name.clone();
    let rest = input.rest;
    let (mut flags, positional) = tokenize_flags(rest);

    if let Some(command) = help::parse(&input, &flags, &positional) {
        return Ok((command, flags));
    }

    let cmd = match cmd_name.as_str() {
        "list" => list::parse(&positional)?,
        "create" => create::parse(&mut flags),
        "config" => config::parse(&positional)?,
        "auth" => auth::parse(&positional, rest)?,
        "send" => send::parse(&mut flags, &positional)?,
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
