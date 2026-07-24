use super::parser::{Command, ParsedInput};
use std::collections::BTreeMap;

pub(super) fn parse(
    input: &ParsedInput<'_>,
    flags: &BTreeMap<String, Option<String>>,
    positional: &[String],
) -> Option<Command> {
    if input.name == "--help" {
        return Some(Command::Help(None));
    }
    if flags.contains_key("help") {
        return Some(if input.top_level_help {
            Command::Help(None)
        } else {
            Command::Help(Some(input.name.clone()))
        });
    }
    if input.name == "help" {
        return Some(Command::Help(positional.first().cloned()));
    }
    None
}
