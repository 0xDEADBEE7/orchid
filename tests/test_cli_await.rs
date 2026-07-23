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
