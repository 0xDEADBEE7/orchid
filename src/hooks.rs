use crate::config::{HookConfiguration, HookEvent, HookPayload};
use crate::log::DiagLogger;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
pub const DEFAULT_OUTPUT_LIMIT: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookResultStatus {
    Succeeded,
    Failed,
    TimedOut,
}

#[derive(Debug, Clone)]
pub struct HookResult {
    pub path: PathBuf,
    pub status: HookResultStatus,
    pub duration: Duration,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub failure: Option<String>,
}

#[derive(Debug, Default)]
pub struct HookRunResult {
    pub scripts: Vec<HookResult>,
}

impl HookRunResult {
    pub fn failed(&self) -> bool {
        self.scripts
            .iter()
            .any(|r| r.status != HookResultStatus::Succeeded)
    }
}

pub struct HookRunner {
    timeout: Duration,
    output_limit: usize,
}

impl Default for HookRunner {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            output_limit: DEFAULT_OUTPUT_LIMIT,
        }
    }
}

impl HookRunner {
    pub fn new(timeout: Duration, output_limit: usize) -> Self {
        Self {
            timeout,
            output_limit,
        }
    }

    pub fn run(
        &self,
        hooks: &HookConfiguration,
        event: HookEvent,
        payload: &HookPayload,
        logger: &DiagLogger,
    ) -> HookRunResult {
        let mut input = match serde_json::to_vec(payload) {
            Ok(value) => value,
            Err(error) => {
                logger.error("hook.serialize", &safe_detail(&error.to_string()));
                return HookRunResult::default();
            }
        };
        input.push(b'\n');
        let mut result = HookRunResult::default();
        for path in hooks.paths_for(event, &payload.working_dir) {
            result
                .scripts
                .push(self.run_one(path, &input, payload, logger));
        }
        result
    }

    fn run_one(
        &self,
        path: PathBuf,
        input: &[u8],
        payload: &HookPayload,
        logger: &DiagLogger,
    ) -> HookResult {
        let started = Instant::now();
        let mut command = Command::new(&path);
        command
            .current_dir(&payload.working_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("ORCHID_EVENT", payload.event.to_string())
            .env("ORCHID_SESSION_ID", &payload.session_id)
            .env("ORCHID_CONFIG_DIR", &payload.config_dir)
            .env("ORCHID_SESSION_DIR", &payload.session_dir)
            .env("ORCHID_WORKING_DIR", &payload.working_dir);
        #[cfg(unix)]
        unsafe {
            command.pre_exec(|| {
                if nix::libc::setpgid(0, 0) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                let failure = safe_detail(&format!("spawn failed: {}", error));
                logger.error("hook.failure", &format!("{}: {}", path.display(), failure));
                return HookResult {
                    path,
                    status: HookResultStatus::Failed,
                    duration: started.elapsed(),
                    exit_code: None,
                    stdout: String::new(),
                    stderr: String::new(),
                    failure: Some(failure),
                };
            }
        };
        self.finish_child(child, path, input, started, logger)
    }

    fn finish_child(
        &self,
        mut child: std::process::Child,
        path: PathBuf,
        input: &[u8],
        started: Instant,
        logger: &DiagLogger,
    ) -> HookResult {
        let output_limit = self.output_limit;
        let stdout = child
            .stdout
            .take()
            .map(|pipe| thread::spawn(move || read_bounded(pipe, output_limit)));
        let stderr = child
            .stderr
            .take()
            .map(|pipe| thread::spawn(move || read_bounded(pipe, output_limit)));
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(input);
        }
        let mut timed_out = false;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) if started.elapsed() >= self.timeout => {
                    timed_out = true;
                    kill_child_tree(&mut child);
                    break child.wait().ok();
                }
                Ok(None) => thread::sleep(Duration::from_millis(5)),
                Err(_) => break None,
            }
        };
        let stdout = stdout
            .and_then(|thread| thread.join().ok())
            .unwrap_or_default();
        let stderr = stderr
            .and_then(|thread| thread.join().ok())
            .unwrap_or_default();
        let (result_status, failure) = if timed_out {
            (
                HookResultStatus::TimedOut,
                Some(format!("timed out after {}ms", self.timeout.as_millis())),
            )
        } else if status.as_ref().map(|s| s.success()).unwrap_or(false) {
            (HookResultStatus::Succeeded, None)
        } else {
            (
                HookResultStatus::Failed,
                Some(format!(
                    "exited with status {}",
                    status
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "unknown".into())
                )),
            )
        };
        if let Some(ref failure) = failure {
            logger.error(
                "hook.failure",
                &format!(
                    "{}: {}; stderr={}",
                    path.display(),
                    failure,
                    safe_detail(&stderr)
                ),
            );
        }
        HookResult {
            path,
            status: result_status,
            duration: started.elapsed(),
            exit_code: status.and_then(|s| s.code()),
            stdout,
            stderr,
            failure,
        }
    }
}

#[cfg(unix)]
fn kill_child_tree(child: &mut std::process::Child) {
    let _ = nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(-(child.id() as i32)),
        nix::sys::signal::Signal::SIGKILL,
    );
    let _ = child.kill();
}

#[cfg(not(unix))]
fn kill_child_tree(child: &mut std::process::Child) {
    let _ = child.kill();
}

fn read_bounded<R: std::io::Read>(mut reader: R, limit: usize) -> String {
    let mut bytes = Vec::new();
    let mut buffer = [0; 8192];
    while bytes.len() < limit {
        let max = (limit - bytes.len()).min(buffer.len());
        match reader.read(&mut buffer[..max]) {
            Ok(0) | Err(_) => break,
            Ok(count) => bytes.extend_from_slice(&buffer[..count]),
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn safe_detail(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .take(4096)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{HookPayload, HookStatus};
    use std::fs;
    use tempfile::tempdir;

    fn payload(dir: &std::path::Path) -> HookPayload {
        HookPayload {
            event: HookEvent::TurnStart,
            session_id: "s1".into(),
            timestamp: chrono::Utc::now(),
            status: HookStatus::Running,
            working_dir: dir.into(),
            config_dir: dir.join("config"),
            session_dir: dir.join("session"),
            error: None,
            reason: None,
        }
    }

    #[cfg(unix)]
    #[test]
    fn runs_in_registration_order_and_continues() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let log = dir.path().join("order");
        for (name, code) in [("one", 0), ("two", 1), ("three", 0)] {
            let path = dir.path().join(name);
            fs::write(&path, format!("#!/bin/sh\nread input\nprintf '%s:%s:%s\\n' '{}' \"$ORCHID_EVENT\" \"$ORCHID_SESSION_ID\" >> '{}'\nexit {}\n", name, log.display(), code)).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let hooks = HookConfiguration {
            turn_start: vec!["one".into(), "two".into(), "three".into()],
            turn_stop: vec![],
        };
        let result = HookRunner::new(Duration::from_secs(2), 1024).run(
            &hooks,
            HookEvent::TurnStart,
            &payload(dir.path()),
            &DiagLogger::noop(),
        );
        assert_eq!(result.scripts.len(), 3);
        assert_eq!(result.scripts[1].status, HookResultStatus::Failed);
        assert_eq!(result.scripts[2].status, HookResultStatus::Succeeded);
        let output = fs::read_to_string(log).unwrap();
        assert!(
            output.find("one:turn-start:s1").unwrap() < output.find("two:turn-start:s1").unwrap()
        );
        assert!(
            output.find("two:turn-start:s1").unwrap() < output.find("three:turn-start:s1").unwrap()
        );
    }

    #[test]
    fn missing_executable_is_recorded_without_stopping_sequence() {
        let dir = tempdir().unwrap();
        let hooks = HookConfiguration {
            turn_start: vec!["missing".into()],
            turn_stop: vec![],
        };
        let result = HookRunner::default().run(
            &hooks,
            HookEvent::TurnStart,
            &payload(dir.path()),
            &DiagLogger::noop(),
        );
        assert_eq!(result.scripts[0].status, HookResultStatus::Failed);
        assert!(result.scripts[0]
            .failure
            .as_deref()
            .unwrap()
            .contains("spawn failed"));
    }

    #[cfg(unix)]
    #[test]
    fn payload_environment_and_working_directory_match() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let script = dir.path().join("inspect");
        fs::write(
            &script,
            "#!/bin/sh\npwd >&2\nenv | grep '^ORCHID_' >&2\ncat\n",
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        let hooks = HookConfiguration {
            turn_start: vec!["inspect".into()],
            turn_stop: vec![],
        };
        let result = HookRunner::new(Duration::from_secs(2), 4096).run(
            &hooks,
            HookEvent::TurnStart,
            &payload(dir.path()),
            &DiagLogger::noop(),
        );
        assert_eq!(result.scripts[0].status, HookResultStatus::Succeeded);
        let value: serde_json::Value =
            serde_json::from_slice(result.scripts[0].stdout.as_bytes()).unwrap();
        assert_eq!(value["event"], "turn-start");
        assert_eq!(value["working_dir"], dir.path().to_string_lossy().as_ref());
        assert!(result.scripts[0]
            .stderr
            .contains(&dir.path().display().to_string()));
        assert!(result.scripts[0].stderr.contains("ORCHID_EVENT=turn-start"));
    }

    #[cfg(unix)]
    #[test]
    fn bounded_stderr_does_not_block_or_exceed_limit() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let script = dir.path().join("noisy");
        fs::write(&script, "#!/bin/sh\nhead -c 200000 /dev/zero >&2\n").unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        let hooks = HookConfiguration {
            turn_start: vec![script.file_name().unwrap().to_string_lossy().into()],
            turn_stop: vec![],
        };
        let result = HookRunner::new(Duration::from_secs(2), 128).run(
            &hooks,
            HookEvent::TurnStart,
            &payload(dir.path()),
            &DiagLogger::noop(),
        );
        assert!(result.scripts[0].stderr.len() <= 128);
    }

    #[cfg(unix)]
    #[test]
    fn direct_spawn_does_not_interpret_shell_metacharacters_and_times_out() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let path = dir.path().join("hook;touch injected");
        fs::write(&path, "#!/bin/sh\nprintf x; sleep 2\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        let hooks = HookConfiguration {
            turn_start: vec![path.file_name().unwrap().to_string_lossy().into()],
            turn_stop: vec![],
        };
        let result = HookRunner::new(Duration::from_millis(30), 1).run(
            &hooks,
            HookEvent::TurnStart,
            &payload(dir.path()),
            &DiagLogger::noop(),
        );
        assert_eq!(result.scripts[0].status, HookResultStatus::TimedOut);
        assert!(result.scripts[0].stdout.len() <= 1);
        assert!(!dir.path().join("injected").exists());
    }
}
