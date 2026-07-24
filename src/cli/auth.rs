use super::{AuthSubcommand, Command};

pub(super) fn parse(positional: &[String], rest: &[String]) -> Result<Command, String> {
    let sub = positional
        .first()
        .ok_or_else(|| "auth requires subcommand: list, validate, or login".to_string())?;
    let command = match sub.as_str() {
        "list" => AuthSubcommand::List,
        "validate" => AuthSubcommand::Validate(
            positional
                .get(1)
                .cloned()
                .ok_or_else(|| "auth validate requires <name>".to_string())?,
        ),
        "login" => AuthSubcommand::Login(
            rest.get(1)
                .cloned()
                .ok_or_else(|| "auth login requires <name>".to_string())?,
        ),
        other => return Err(format!("unknown auth subcommand: {}", other)),
    };
    Ok(Command::Auth(command))
}
