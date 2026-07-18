use crate::JsonError;
use serde_json::Value;
use std::io::Write;

pub fn print_json(value: &Value) -> Result<(), String> {
    let json =
        serde_json::to_string(value).map_err(|e| format!("json serialization failed: {}", e))?;
    println!("{}", json);
    Ok(())
}

pub fn print_error(err: &JsonError) -> Result<(), String> {
    let json =
        serde_json::to_string(err).map_err(|e| format!("error serialization failed: {}", e))?;

    if let Err(e) = writeln!(std::io::stderr(), "{}", json) {
        if e.kind() != std::io::ErrorKind::BrokenPipe {
            return Err(format!("failed to write to stderr: {}", e));
        }
    }

    Ok(())
}
