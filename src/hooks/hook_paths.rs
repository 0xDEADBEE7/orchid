use crate::config::Settings;
use std::path::{Path, PathBuf};

pub(super) fn executable(settings: &Settings, script: &str) -> PathBuf {
    let path = Path::new(script);
    let local = settings.root.join(path);
    if path.is_absolute() || path.components().count() > 1 || is_executable(&local) {
        local.canonicalize().unwrap_or(local)
    } else {
        path.to_path_buf()
    }
}

pub(super) fn working_dir(settings: &Settings, value: Option<&str>) -> PathBuf {
    let Some(value) = value else {
        return settings.root.clone();
    };
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        settings.root.join(path)
    }
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}
