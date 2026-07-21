use std::fs;

use super::{resource_name, ConfigDir, ResourceLoadError};

/// Load a named UTF-8 Markdown prompt from the selected config directory.
pub fn load(dir: &ConfigDir, name: &str) -> Result<String, ResourceLoadError> {
    let path = dir.prompts_path().join(format!("{}.md", name));
    resource_name(name).map_err(|message| ResourceLoadError::Invalid {
        path: path.clone(),
        message,
    })?;
    if !path.is_file() {
        return Err(ResourceLoadError::Missing {
            kind: "prompt",
            path,
        });
    }
    fs::read_to_string(&path).map_err(|source| ResourceLoadError::Read { path, source })
}
