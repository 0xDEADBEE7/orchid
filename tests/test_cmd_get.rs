use orchid::cli::{parse_args, Command};
use orchid::cmd::get;
use orchid::SessionStore;
use std::fs;
use tempfile::TempDir;

fn args(values: &[&str]) -> Vec<String> { values.iter().map(|v| (*v).to_string()).collect() }

#[test]
fn parses_selectors_in_any_order_and_rejects_empty_selection() {
    let (command, _) = parse_args(&args(&["get", "0123456789abcdef0123456789abcdef", "--state", "--conversation", "--metadata"])).unwrap();
    assert_eq!(command, Command::Get {
        id: "0123456789abcdef0123456789abcdef".to_string(),
        conversation: true,
        last_message: false,
        metadata: true,
        state: true,
    });
    assert!(parse_args(&args(&["get", "0123456789abcdef0123456789abcdef"])).is_err());
    assert!(parse_args(&args(&["get", "0123456789abcdef0123456789abcdef", "--logs"])).is_err());
}

#[test]
fn get_binary_accepts_config_before_and_after_command() {
    let temp = TempDir::new().unwrap();
    let store = SessionStore::with_config_dir(temp.path()).unwrap();
    let session = store.create(Some("task".into()), None, None).unwrap();
    let binary = env!("CARGO_BIN_EXE_orchid");

    for args in [
        vec!["--config", temp.path().to_str().unwrap(), "get", &session.id, "--state"],
        vec!["get", &session.id, "--state", "--config", temp.path().to_str().unwrap()],
    ] {
        let output = std::process::Command::new(binary).args(&args).output().unwrap();
        assert!(output.status.success(), "args={args:?}, stderr={:?}", output.stderr);
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["state"]["status"], "idle");
    }
}

#[test]
fn get_reads_selected_resources_in_stable_shape() {
    let temp = TempDir::new().unwrap();
    let store = SessionStore::with_config_dir(temp.path()).unwrap();
    let session = store.create(Some("task".into()), None, None).unwrap();
    fs::write(store.transcript_path(&session.id), "{\"event\":1}\n\n{\"event\":2}\n").unwrap();

    let value = get(&session.id, true, false, true, true, temp.path()).unwrap();
    assert_eq!(value["conversation"], serde_json::json!([{ "event": 1 }, { "event": 2 }]));
    assert_eq!(value["metadata"]["id"], session.id);
    assert_eq!(value["state"]["status"], "idle");
    assert_eq!(value.as_object().unwrap().keys().collect::<Vec<_>>(), vec!["conversation", "metadata", "state"]);
}

#[test]
fn get_reports_invalid_and_missing_or_malformed_resources() {
    let temp = TempDir::new().unwrap();
    assert!(get("not-an-id", false, false, true, false, temp.path()).unwrap_err().contains("invalid session ID"));
    assert!(get("0123456789abcdef0123456789abcdef", false, false, true, false, temp.path()).unwrap_err().contains("missing"));
    let store = SessionStore::with_config_dir(temp.path()).unwrap();
    let session = store.create(None, None, None).unwrap();
    fs::write(store.state_path(&session.id), "not json").unwrap();
    assert!(get(&session.id, false, false, false, true, temp.path()).unwrap_err().contains("invalid state JSON"));
}

#[test]
fn last_message_selects_latest_assistant_message_not_final_event() {
    let temp = TempDir::new().unwrap();
    let store = SessionStore::with_config_dir(temp.path()).unwrap();
    let session = store.create(Some("task".into()), None, None).unwrap();
    fs::write(
        store.transcript_path(&session.id),
        "{\"type\":\"message\",\"message\":{\"role\":\"user\",\"content\":\"start\"}}\n{\"type\":\"message\",\"message\":{\"role\":\"assistant\",\"content\":\"answer\"}}\n{\"type\":\"tool_result\",\"tool_result\":{}}\n",
    )
    .unwrap();

    let value = get(&session.id, false, true, false, false, temp.path()).unwrap();
    assert_eq!(value["last_message"]["message"]["content"], "answer");
}

#[test]
fn get_reads_running_session_without_mutation() {
    let temp = TempDir::new().unwrap();
    let store = SessionStore::with_config_dir(temp.path()).unwrap();
    let session = store.create(None, None, None).unwrap();
    store.update(&session.id, orchid::SessionUpdate {
        status: Some(orchid::types::Status::Running),
        ..Default::default()
    }).unwrap();
    fs::write(store.transcript_path(&session.id), "{\"type\":\"message\",\"message\":{\"role\":\"assistant\",\"content\":\"partial\"}}\n").unwrap();

    let value = get(&session.id, false, true, false, true, temp.path()).unwrap();
    assert_eq!(value["last_message"]["message"]["content"], "partial");
    assert_eq!(value["state"]["status"], "running");
}
#[test]
fn get_does_not_modify_resources() {
    let temp = TempDir::new().unwrap();
    let store = SessionStore::with_config_dir(temp.path()).unwrap();
    let session = store.create(None, None, None).unwrap();
    fs::write(store.transcript_path(&session.id), "{\"event\":true}\n").unwrap();
    let before = [
        fs::read(store.metadata_path(&session.id)).unwrap(),
        fs::read(store.state_path(&session.id)).unwrap(),
        fs::read(store.transcript_path(&session.id)).unwrap(),
    ];
    get(&session.id, true, false, true, true, temp.path()).unwrap();
    assert_eq!(fs::read(store.metadata_path(&session.id)).unwrap(), before[0]);
    assert_eq!(fs::read(store.state_path(&session.id)).unwrap(), before[1]);
    assert_eq!(fs::read(store.transcript_path(&session.id)).unwrap(), before[2]);
}
