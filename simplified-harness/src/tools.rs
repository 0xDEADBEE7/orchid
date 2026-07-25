use crate::{config::Settings, model::Event};
use serde_json::Value;
use std::process::Command;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub fn read(settings: &Settings, paths: &[String]) -> io::Result<Value> {
    require(settings, "fs_read")?;
    if paths.is_empty() {
        return Err(invalid("fs_read requires a non-empty paths array"));
    }
    let mut result = serde_json::Map::new();
    for input in paths {
        match safe_path(settings, input)
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

pub fn edit(settings: &Settings, path: &str, edits: &Value) -> io::Result<Value> {
    require(settings, "fs_edit")?;
    let path = safe_path(settings, path)?;
    let edits = edits
        .as_array()
        .ok_or_else(|| invalid("fs_edit edits must be an array"))?;
    if edits.is_empty() {
        return Err(invalid("fs_edit edits must not be empty"));
    }
    let content = edits
        .iter()
        .try_fold(fs::read_to_string(&path).unwrap_or_default(), apply_edit)?;
    fs::write(&path, content)?;
    Ok(serde_json::json!({"path": path, "edits_applied": edits.len()}))
}

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

pub fn event(name: &str, input: Value, result: Value) -> [Event; 2] {
    [
        Event::ToolCall {
            name: name.into(),
            input,
        },
        Event::ToolResult { content: result },
    ]
}

pub fn bash(settings: &Settings, command: &str) -> io::Result<Value> {
    require(settings, "bash")?;
    let output = Command::new("bash")
        .arg("-c")
        .arg(command)
        .current_dir(&settings.root)
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

fn require(settings: &Settings, tool: &str) -> io::Result<()> {
    if settings
        .policy
        .tools()
        .iter()
        .any(|allowed| allowed == tool)
    {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("tool denied by policy: {tool}"),
        ))
    }
}

fn safe_path(settings: &Settings, input: &str) -> io::Result<PathBuf> {
    let base = Path::new(settings.root.as_path());
    let path = if Path::new(input).is_absolute() {
        PathBuf::from(input)
    } else {
        base.join(input)
    };
    let base = base.canonicalize()?;
    let candidate = path
        .parent()
        .unwrap_or(&path)
        .canonicalize()?
        .join(path.file_name().ok_or_else(|| invalid("path is empty"))?);
    let allowed = settings.policy.paths().is_empty()
        || settings.policy.paths().iter().any(|path| {
            let path = base
                .join(path)
                .canonicalize()
                .unwrap_or_else(|_| base.join(path));
            candidate.starts_with(path)
        });
    if candidate.starts_with(&base) && allowed {
        Ok(candidate)
    } else {
        Err(invalid("path is outside policy scope"))
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

pub fn command(settings: &Settings, args: &[String]) -> io::Result<String> {
    let name = args
        .first()
        .ok_or_else(|| invalid("tool requires a name"))?;
    let input = args.get(1..).unwrap_or_default().join(" ");
    let result = run_named(settings, name, &input)?;
    serde_json::to_string(&result).map_err(io::Error::other)
}

fn run_named(settings: &Settings, name: &str, input: &str) -> io::Result<Value> {
    match name {
        "bash" => bash(settings, input),
        "fs_read" => read(settings, &[input.to_owned()]),
        "fs_edit" => edit_input(settings, input),
        _ => Err(invalid("unknown tool")),
    }
}

fn edit_input(settings: &Settings, input: &str) -> io::Result<Value> {
    let (path, content) = input
        .split_once(' ')
        .ok_or_else(|| invalid("fs_edit requires path and content"))?;
    edit(
        settings,
        path,
        &serde_json::json!([{"old_string":"","new_string":content}]),
    )
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
