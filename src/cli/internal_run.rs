use super::Command;

pub(super) fn parse(positional: &[String]) -> Result<Command, String> {
    let id = positional
        .first()
        .cloned()
        .ok_or_else(|| "__run requires <id>".to_string())?;
    Ok(Command::InternalRun { id })
}
