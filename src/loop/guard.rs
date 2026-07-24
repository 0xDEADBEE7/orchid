use crate::config::{HookConfiguration, HookEvent, HookStatus};
use crate::hooks::HookRunner;
use crate::log::DiagLogger;
use crate::r#loop::lifecycle;
use crate::types::Status;
use std::path::{Path, PathBuf};

pub struct RunGuard<'a> {
    session_id: String,
    config_dir: PathBuf,
    working_dir: PathBuf,
    session_dir: PathBuf,
    hooks: HookConfiguration,
    logger: &'a DiagLogger,
    finished: bool,
}

impl<'a> RunGuard<'a> {
    pub fn new(session_id: &str, config_dir: &Path) -> Self {
        Self::with_hooks(
            session_id,
            config_dir,
            Path::new("."),
            Path::new("."),
            HookConfiguration {
                turn_start: Vec::new(),
                turn_stop: Vec::new(),
            },
            Box::leak(Box::new(DiagLogger::noop())),
        )
    }

    pub fn with_hooks(
        session_id: &str,
        config_dir: &Path,
        working_dir: &Path,
        session_dir: &Path,
        hooks: HookConfiguration,
        logger: &'a DiagLogger,
    ) -> Self {
        Self {
            session_id: session_id.to_string(),
            config_dir: config_dir.to_path_buf(),
            working_dir: working_dir.to_path_buf(),
            session_dir: session_dir.to_path_buf(),
            hooks,
            logger,
            finished: false,
        }
    }
    pub fn finish(
        &mut self,
        status: Status,
        error: Option<String>,
        reason: Option<String>,
    ) -> Result<(), String> {
        if self.finished {
            return Ok(());
        }
        self.finished = true;
        let state_result = match status {
            Status::Idle => lifecycle::on_run_end(&self.session_id, &self.config_dir),
            Status::Cancelled => lifecycle::on_run_end_with_status(
                &self.session_id,
                Status::Cancelled,
                &self.config_dir,
            ),
            _ => lifecycle::on_run_failed(&self.session_id, &self.config_dir),
        };
        let hook_status = match status {
            Status::Idle => HookStatus::Succeeded,
            Status::Cancelled => HookStatus::Cancelled,
            _ => HookStatus::Failed,
        };
        let payload = lifecycle::hook_payload(
            HookEvent::TurnStop,
            &self.session_id,
            hook_status,
            &self.working_dir,
            &self.config_dir,
            &self.session_dir,
            error,
            reason,
        );
        let _ = HookRunner::default().run(&self.hooks, HookEvent::TurnStop, &payload, self.logger);
        state_result
    }
}

impl Drop for RunGuard<'_> {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.finish(
                Status::Failed,
                Some("unexpected run exit".into()),
                Some("guard fallback".into()),
            );
        }
    }
}
