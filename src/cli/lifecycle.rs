use super::Command;

pub(super) fn parse(positional: &[String], command: &str) -> Result<Command, String> {
    let id = positional
        .first()
        .cloned()
        .ok_or_else(|| format!("{} requires <id>", command))?;
    Ok(match command {
        "stop" => Command::Stop(id),
        "kill" => Command::Kill(id),
        _ => unreachable!(),
    })
}
