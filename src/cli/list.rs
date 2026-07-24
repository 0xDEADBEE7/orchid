use super::Command;

pub(super) fn parse(positional: &[String]) -> Result<Command, String> {
    let resource = positional.first().cloned();
    if let Some(name) = &resource {
        if !matches!(
            name.as_str(),
            "sessions" | "connections" | "policies" | "prompts" | "auth"
        ) {
            return Err(format!("unknown list resource: {}", name));
        }
    }
    Ok(Command::List(resource))
}
