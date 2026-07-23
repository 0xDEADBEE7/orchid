use orchid::session::SessionUpdate;
use orchid::types::Status;
use orchid::SessionStore;
use std::process::Command;
use tempfile::TempDir;

fn run_await(config_dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    let binary = env!("CARGO_BIN_EXE_orchid");
    Command::new(binary)
        .arg("await")
        .args(args)
        .arg("--config")
        .arg(config_dir)
        .output()
        .unwrap()
}

#[test]
#[serial_test::serial]
fn await_binary_reports_completion_as_json() {
    let temp = TempDir::new().unwrap();
    let store = SessionStore::with_config_dir(temp.path()).unwrap();
    let session = store.create(None, None, None).unwrap();

    let output = run_await(temp.path(), &[&session.id]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["completed"][0]["id"], session.id);
    assert_eq!(value["completed"][0]["status"], "idle");
}

#[test]
#[serial_test::serial]
fn await_binary_reports_timeout_with_exit_code_two() {
    let temp = TempDir::new().unwrap();
    let store = SessionStore::with_config_dir(temp.path()).unwrap();
    let session = store.create(None, None, None).unwrap();
    store
        .update(
            &session.id,
            SessionUpdate {
                status: Some(Status::Running),
                ..Default::default()
            },
        )
        .unwrap();

    let output = run_await(temp.path(), &[&session.id, "--timeout=0", "--interval=0"]);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value, serde_json::json!({"completed": [], "timed_out": true}));
    assert_eq!(store.state(&session.id).unwrap().status, Status::Running);
}

#[test]
fn await_binary_reports_invalid_usage_as_json_error() {
    let temp = TempDir::new().unwrap();
    let output = run_await(temp.path(), &["not-a-session-id"]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"], "command_error");
    assert!(error["message"].as_str().unwrap().contains("invalid session ID"));
}

#[test]
#[serial_test::serial]
fn await_binary_reports_missing_state_as_json_error_without_mutation() {
    let temp = TempDir::new().unwrap();
    let store = SessionStore::with_config_dir(temp.path()).unwrap();
    let session = store.create(None, None, None).unwrap();
    let metadata_before = std::fs::read(store.metadata_path(&session.id)).unwrap();
    std::fs::remove_file(store.state_path(&session.id)).unwrap();

    let output = run_await(temp.path(), &[&session.id]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"], "command_error");
    assert!(error["message"].as_str().unwrap().contains("session state is missing"));
    assert_eq!(std::fs::read(store.metadata_path(&session.id)).unwrap(), metadata_before);
    assert!(!store.state_path(&session.id).exists());
}

#[test]
#[serial_test::serial]
fn await_binary_reports_malformed_state_as_json_error_without_mutation() {
    let temp = TempDir::new().unwrap();
    let store = SessionStore::with_config_dir(temp.path()).unwrap();
    let session = store.create(None, None, None).unwrap();
    let metadata_before = std::fs::read(store.metadata_path(&session.id)).unwrap();
    let malformed = b"not json";
    std::fs::write(store.state_path(&session.id), malformed).unwrap();

    let output = run_await(temp.path(), &[&session.id]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"], "command_error");
    assert!(error["message"].as_str().unwrap().contains("invalid state JSON"));
    assert_eq!(std::fs::read(store.state_path(&session.id)).unwrap(), malformed);
    assert_eq!(std::fs::read(store.metadata_path(&session.id)).unwrap(), metadata_before);
}

#[cfg(unix)]
#[test]
#[serial_test::serial]
fn await_ctrl_c_does_not_cancel_session() {
    let temp = TempDir::new().unwrap();
    let store = SessionStore::with_config_dir(temp.path()).unwrap();
    let session = store.create(None, None, None).unwrap();
    store
        .update(
            &session.id,
            SessionUpdate {
                status: Some(Status::Running),
                ..Default::default()
            },
        )
        .unwrap();
    let metadata_before = std::fs::read(store.metadata_path(&session.id)).unwrap();
    let state_before = std::fs::read(store.state_path(&session.id)).unwrap();
    let binary = env!("CARGO_BIN_EXE_orchid");
    let mut child = Command::new(binary)
        .arg("await")
        .arg(&session.id)
        .arg("--timeout=30")
        .arg("--interval=1")
        .arg("--config")
        .arg(temp.path())
        .spawn()
        .unwrap();

    std::thread::sleep(std::time::Duration::from_millis(50));
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(child.id() as i32),
        nix::sys::signal::Signal::SIGINT,
    )
    .unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(!output.status.success());
    assert_eq!(output.status.code(), None);
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert_eq!(std::fs::read(store.state_path(&session.id)).unwrap(), state_before);
    assert_eq!(std::fs::read(store.metadata_path(&session.id)).unwrap(), metadata_before);
}
#[test]
#[serial_test::serial]
fn await_binary_accepts_config_before_and_after_command() {
    let temp = TempDir::new().unwrap();
    let store = SessionStore::with_config_dir(temp.path()).unwrap();
    let session = store.create(None, None, None).unwrap();
    let binary = env!("CARGO_BIN_EXE_orchid");

    for args in [
        vec!["--config", temp.path().to_str().unwrap(), "await", &session.id],
        vec!["await", &session.id, "--config", temp.path().to_str().unwrap()],
    ] {
        let output = Command::new(binary).args(&args).output().unwrap();
        assert_eq!(output.status.code(), Some(0), "args={args:?}");
        assert!(serde_json::from_slice::<serde_json::Value>(&output.stdout).is_ok());
        assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    }
}
