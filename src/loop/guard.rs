use crate::r#loop::lifecycle;

/// RAII guard that ensures `on_run_end` is called when a run completes or fails.
///
/// Call `disarm()` after a successful run to prevent the cleanup from firing.
use std::path::Path;

pub struct RunGuard<'a> {
    session_id: &'a str,
    config_dir: &'a Path,
    disarmed: bool,
}

impl<'a> RunGuard<'a> {
    pub fn new(session_id: &'a str, config_dir: &'a Path) -> Self {
        Self {
            session_id,
            config_dir,
            disarmed: false,
        }
    }

    pub fn disarm(&mut self) {
        self.disarmed = true;
    }
}

impl Drop for RunGuard<'_> {
    fn drop(&mut self) {
        if !self.disarmed {
            let _ = lifecycle::on_run_end(self.session_id, self.config_dir);
        }
    }
}
