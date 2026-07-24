use std::collections::BTreeMap;

use super::Command;

pub(super) fn parse(flags: &mut BTreeMap<String, Option<String>>) -> Result<Command, String> {
    let id = flags
        .remove("id")
        .flatten()
        .ok_or_else(|| "set requires --id".to_string())?;
    let label = flags.remove("label").flatten();
    let working_dir = flags.remove("working-dir").flatten();
    let restrictions = flags
        .remove("restriction")
        .map(|v| v.map(|s| vec![s]))
        .unwrap_or_default();
    Ok(Command::Set {
        id,
        label,
        working_dir,
        restrictions,
    })
}
