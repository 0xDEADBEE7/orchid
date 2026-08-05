use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub(super) fn depth(root: &Path, session_id: &str) -> u32 {
    fs::read_to_string(root.join("sessions").join(session_id).join(".hook-depth"))
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

pub(super) struct HookDepth {
    path: PathBuf,
}

impl HookDepth {
    pub(super) fn enter(root: &Path, session_id: &str) -> io::Result<Self> {
        let dir = root.join("sessions").join(session_id);
        fs::create_dir_all(&dir)?;
        let path = dir.join(".hook-depth");
        fs::write(&path, (depth(root, session_id) + 1).to_string())?;
        Ok(Self { path })
    }
}

impl Drop for HookDepth {
    fn drop(&mut self) {
        let depth = fs::read_to_string(&self.path)
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(1);
        if depth <= 1 {
            let _ = fs::remove_file(&self.path);
        } else {
            let _ = fs::write(&self.path, (depth - 1).to_string());
        }
    }
}
