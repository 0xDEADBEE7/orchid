use std::collections::BTreeMap;

use super::parser::{Command, VALUE_FLAGS};

pub(super) fn parse(
    flags: &mut BTreeMap<String, Option<String>>,
    positional: &[String],
) -> Result<Command, String> {
    let message = positional
        .first()
        .cloned()
        .ok_or_else(|| "send requires a message".to_string())?;
    let id = flags.remove("id").flatten();
    let await_completion = flags.remove("await").is_some();
    let label = flags.remove("label").flatten();
    let policy = flags.remove("policy").flatten();
    let prompt = flags.remove("prompt").flatten();
    let working_dir = flags.remove("working-dir").flatten();

    if let Some(unknown) = flags
        .iter()
        .find(|(key, _)| !VALUE_FLAGS.contains(&key.as_str()))
        .map(|(key, _)| key.as_str())
    {
        return Err(format!("unknown flag: --{}", unknown));
    }

    Ok(Command::Send {
        id,
        message,
        await_completion,
        label,
        working_dir,
        policy,
        prompt,
    })
}
