use super::Command;
use std::collections::BTreeMap;

pub(super) fn parse(
    flags: &mut BTreeMap<String, Option<String>>,
    positional: &[String],
) -> Result<Command, String> {
    let id = positional
        .first()
        .cloned()
        .ok_or_else(|| "get requires <session ID>".to_string())?;
    let conversation = flags.remove("conversation").is_some();
    let last_message = flags.remove("last-message").is_some();
    let metadata = flags.remove("metadata").is_some();
    let state = flags.remove("state").is_some();
    if !conversation && !last_message && !metadata && !state {
        return Err(
            "get requires at least one selector: --conversation, --last-message, --metadata, or --state"
                .to_string(),
        );
    }
    if let Some(unknown) = flags.keys().next() {
        return Err(format!("unknown flag: --{}", unknown));
    }
    if positional.len() > 1 {
        return Err("get accepts exactly one session ID".to_string());
    }
    Ok(Command::Get {
        id,
        conversation,
        last_message,
        metadata,
        state,
    })
}
