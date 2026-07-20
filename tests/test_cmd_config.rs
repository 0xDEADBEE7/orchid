#[test]
fn test_fs_edit_rejects_legacy_single_edit_fields() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file.txt");
    std::fs::write(&path, "before").unwrap();
    let result = orchid::tools::fs_edit::execute(
        serde_json::json!({
            "path": path.display().to_string(),
            "old_string": "before",
            "new_string": "after"
        }),
        dir.path().to_str().unwrap(),
        true,
        &globset::GlobSet::empty(),
        &globset::GlobSet::empty(),
        &[],
    );
    assert!(result.is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "before");
}

#[test]
fn test_fs_edit_rejects_empty_edits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file.txt");
    std::fs::write(&path, "before").unwrap();
    let result = orchid::tools::fs_edit::execute(
        serde_json::json!({"path": path.display().to_string(), "edits": []}),
        dir.path().to_str().unwrap(),
        true,
        &globset::GlobSet::empty(),
        &globset::GlobSet::empty(),
        &[],
    );
    assert!(result.unwrap_err().contains("must not be empty"));
}

use orchid::cmd::{config_list, config_show};
use std::fs;

#[test]
fn test_session_stores_are_isolated_by_config_directory() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let first_store = orchid::SessionStore::with_config_dir(first.path()).unwrap();
    let second_store = orchid::SessionStore::with_config_dir(second.path()).unwrap();

    let first_session = first_store
        .create(Some("first".into()), None, None)
        .unwrap();
    let second_session = second_store
        .create(Some("second".into()), None, None)
        .unwrap();

    assert!(first_store.get(&first_session.id).is_ok());
    assert!(first_store.get(&second_session.id).is_err());
    assert!(second_store.get(&second_session.id).is_ok());
    assert!(second_store.get(&first_session.id).is_err());
}

#[test]
fn test_missing_config_resources_do_not_create_partial_session() {
    let dir = tempfile::tempdir().unwrap();
    let store = orchid::SessionStore::with_config_dir(dir.path()).unwrap();
    let session = store.create(None, None, None).unwrap();
    assert!(store.metadata_path(&session.id).is_file());
    assert!(store.state_path(&session.id).is_file());
}
#[test]
fn test_config_list_resources() {
    let dir = tempfile::tempdir().unwrap();
    for kind in ["connections", "policies", "prompts"] {
        fs::create_dir_all(dir.path().join(kind)).unwrap();
    }
    fs::write(dir.path().join("connections/local.json"), "{}").unwrap();
    let result = config_list(dir.path()).unwrap();
    assert!(result.get("connections").is_some());
}

#[test]
fn test_config_show_root() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("config.json"), r#"{"policy":"default"}"#).unwrap();
    let result = config_show(dir.path(), "root").unwrap();
    assert_eq!(result["policy"], "default");
}
