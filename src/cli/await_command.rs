use super::Command;
use std::collections::BTreeMap;

pub(super) fn parse(
    flags: &mut BTreeMap<String, Option<String>>,
    positional: Vec<String>,
) -> Result<Command, String> {
    if positional.is_empty() {
        return Err("await requires at least one session ID".to_string());
    }
    let timeout = parse_nonnegative_float(flags.remove("timeout").flatten(), "timeout")?;
    let interval = parse_nonnegative_float(flags.remove("interval").flatten(), "interval")?;
    if let Some(unknown) = flags
        .keys()
        .find(|key| !matches!(key.as_str(), "timeout" | "interval"))
    {
        return Err(format!("unknown flag: --{}", unknown));
    }
    Ok(Command::Await {
        ids: positional,
        timeout,
        interval,
    })
}

fn parse_nonnegative_float(value: Option<String>, name: &str) -> Result<f64, String> {
    let value = value.unwrap_or_else(|| if name == "timeout" { "60" } else { "2" }.to_string());
    let parsed = value
        .parse::<f64>()
        .map_err(|_| format!("invalid {} value: {}", name, value))?;
    if !parsed.is_finite() || parsed < 0.0 {
        return Err(format!("invalid {} value: {}", name, value));
    }
    Ok(parsed)
}
