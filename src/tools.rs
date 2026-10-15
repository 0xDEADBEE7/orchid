//! Provides the tools functionality.
use crate::config::Settings;
use serde_json::Value;
use std::process::Command;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Largest tool output, in characters, kept in a session or sent to a model.
const MAX_OUTPUT: usize = 50_000;

/// Renders a tool result for a model: strings as-is, anything else as JSON.
pub fn text(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_owned)
}

/// Keeps the head and tail of oversized tool output.
pub fn clip(value: Value) -> Value {
    let text = text(&value);
    let len = text.chars().count();
    if len <= MAX_OUTPUT {
        return value;
    }
    let head: String = text.chars().take(MAX_OUTPUT / 2).collect();
    let tail: String = text.chars().skip(len - MAX_OUTPUT / 2).collect();
    let cut = len - MAX_OUTPUT;
    Value::String(format!(
        "{head}\n[... {cut} characters truncated ...]\n{tail}"
    ))
}

/// Reads files relative to a base directory.
pub fn read_from(settings: &Settings, paths: &[String], base: &Path) -> io::Result<Value> {
    require(settings, "fs_read")?;
    if paths.is_empty() {
        return Err(invalid("fs_read requires a non-empty paths array"));
    }
    let mut result = serde_json::Map::new();
    for input in paths {
        match safe_path_at(input, base)
            .and_then(|path| fs::read_to_string(&path).map(|s| (path, s)))
        {
            Ok((path, content)) => {
                result.insert(input.clone(), Value::String(content));
                let _ = path;
            }
            Err(error) if paths.len() > 1 => {
                result.insert(
                    input.clone(),
                    serde_json::json!({"error": error.to_string()}),
                );
            }
            Err(error) => return Err(error),
        }
    }
    Ok(Value::Object(result))
}

/// Applies edits to a file relative to a base directory.
pub fn edit_from(settings: &Settings, path: &str, edits: &Value, base: &Path) -> io::Result<Value> {
    require(settings, "fs_edit")?;
    let path = safe_path_at(path, base)?;
    let edits = edits
        .as_array()
        .ok_or_else(|| invalid("fs_edit edits must be an array"))?;
    if edits.is_empty() {
        return Err(invalid("fs_edit edits must not be empty"));
    }
    let content = edits.iter().enumerate().try_fold(
        fs::read_to_string(&path).unwrap_or_default(),
        |content, (index, edit)| {
            apply_edit(content, edit)
                .map_err(|error| invalid(&format!("fs_edit edit[{index}] failed: {error}")))
        },
    )?;
    fs::write(&path, content)?;
    Ok(serde_json::json!({"path": path, "edits_applied": edits.len()}))
}

/// Applies one text edit.
fn apply_edit(content: String, edit: &Value) -> io::Result<String> {
    let old = edit
        .get("old_string")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let new = edit
        .get("new_string")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let replace_all = edit
        .get("replace_all")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let count = content.matches(old).count();
    if count == 0 && !old.is_empty() {
        return Err(invalid("edit pattern not found"));
    }
    if count > 1 && !replace_all {
        return Err(invalid("edit matches multiple locations"));
    }
    Ok(if old.is_empty() {
        new.to_string()
    } else if replace_all {
        content.replace(old, new)
    } else {
        content.replacen(old, new, 1)
    })
}

/// Runs a shell command in a working directory.
pub fn bash_in(settings: &Settings, command: &str, working_dir: &Path) -> io::Result<Value> {
    require(settings, "bash")?;
    let output = Command::new("bash")
        .arg("-c")
        .arg(command)
        .current_dir(working_dir)
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(Value::String(if stderr.is_empty() {
        stdout.into()
    } else if stdout.is_empty() {
        stderr.into()
    } else {
        format!("{}{}", stdout, stderr)
    }))
}

/// Checks whether a tool is allowed.
fn require(settings: &Settings, tool: &str) -> io::Result<()> {
    if settings.agent.tools.iter().any(|allowed| allowed == tool) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("tool not enabled for this agent: {tool}"),
        ))
    }
}

/// Resolves a path, which must stay inside the session's working directory.
/// Narrower rules (e.g. allowed subdirectories) belong in an on-tool-call hook.
fn safe_path_at(input: &str, base: &Path) -> io::Result<PathBuf> {
    let path = base.join(input);
    let base = base.canonicalize()?;
    let candidate = path
        .parent()
        .unwrap_or(&path)
        .canonicalize()?
        .join(path.file_name().ok_or_else(|| invalid("path is empty"))?);
    if candidate.starts_with(&base) {
        Ok(candidate)
    } else {
        Err(invalid("path is outside the working directory"))
    }
}

/// Creates an invalid-input error.
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
