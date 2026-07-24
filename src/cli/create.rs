use std::collections::BTreeMap;

use super::Command;

pub(super) fn parse(flags: &mut BTreeMap<String, Option<String>>) -> Command {
    let label = flags.remove("label").flatten();
    let policy = flags.remove("policy").flatten();
    let prompt = flags.remove("prompt").flatten();
    let working_dir = flags.remove("working-dir").flatten();
    let restrictions = flags
        .remove("restriction")
        .map(|v| v.map(|s| vec![s]))
        .unwrap_or_default();
    Command::Create {
        label,
        working_dir,
        policy,
        prompt,
        restrictions,
    }
}
