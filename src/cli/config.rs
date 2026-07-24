use super::{Command, ConfigSubcommand};

pub(super) fn parse(positional: &[String]) -> Result<Command, String> {
    if positional.is_empty() {
        return Err("config requires subcommand: validate, list, or show".to_string());
    }
    let command = match positional[0].as_str() {
        "validate" => ConfigSubcommand::Validate,
        "list" => ConfigSubcommand::List,
        "show" => ConfigSubcommand::Show(
            positional
                .get(1)
                .cloned()
                .ok_or_else(|| "config show requires <resource>".to_string())?,
        ),
        "use" => ConfigSubcommand::Use(
            positional
                .get(1)
                .cloned()
                .ok_or_else(|| "config use requires <policy>".to_string())?,
        ),
        other => return Err(format!("unknown config subcommand: {}", other)),
    };
    Ok(Command::Config(command))
}
