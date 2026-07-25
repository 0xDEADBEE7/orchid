use crate::config::Settings;
use std::{
    io,
    process::{Command, Stdio},
    time::Duration,
};

pub fn run(settings: &Settings, event: &str, session_id: &str) -> io::Result<()> {
    for hook in &settings.policy.hooks {
        let mut child = Command::new(hook)
            .arg(event)
            .arg(session_id)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        std::thread::sleep(Duration::from_millis(10));
        if child.try_wait()?.is_none() {
            let _ = child.kill();
        }
    }
    Ok(())
}
