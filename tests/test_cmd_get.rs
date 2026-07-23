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
        metadata: true,
        state: true,
    });
    assert!(parse_args(&args(&["get", "0123456789abcdef0123456789abcdef"])).is_err());
    assert!(parse_args(&args(&["get", "0123456789abcdef0123456789abcdef", "--logs"])).is_err());
}

#[test]
fn get_reads_selected_resources_in_stable_shape() {
    let temp = TempDir::new().unwrap();
    let store = SessionStore::with_config_dir(temp.path()).unwrap();
    let session = store.create(Some("task".into()), None, None).unwrap();
    fs::write(store.transcript_path(&session.id), "{\"event\":1}\n\n{\"event\":2}\n").unwrap();

    let value = get(&session.id, true, true, true, temp.path()).unwrap();
    assert_eq!(value["conversation"], serde_json::json!([{ "event": 1 }, { "event": 2 }]));
    assert_eq!(value["metadata"]["id"], session.id);
    assert_eq!(value["state"]["status"], "idle");
    assert_eq!(value.as_object().unwrap().keys().collect::<Vec<_>>(), vec!["conversation", "metadata", "state"]);
}

#[test]
fn get_reports_invalid_and_missing_or_malformed_resources() {
    let temp = TempDir::new().unwrap();
    assert!(get("not-an-id", false, true, false, temp.path()).unwrap_err().contains("invalid session ID"));
    assert!(get("0123456789abcdef0123456789abcdef", false, true, false, temp.path()).unwrap_err().contains("missing"));
    let store = SessionStore::with_config_dir(temp.path()).unwrap();
    let session = store.create(None, None, None).unwrap();
    fs::write(store.state_path(&session.id), "not json").unwrap();
    assert!(get(&session.id, false, false, true, temp.path()).unwrap_err().contains("invalid state JSON"));
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
    get(&session.id, true, true, true, temp.path()).unwrap();
    assert_eq!(fs::read(store.metadata_path(&session.id)).unwrap(), before[0]);
    assert_eq!(fs::read(store.state_path(&session.id)).unwrap(), before[1]);
    assert_eq!(fs::read(store.transcript_path(&session.id)).unwrap(), before[2]);
}
