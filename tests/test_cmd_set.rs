use orchid::cmd::set;
use orchid::SessionStore as Store;
mod support;
use support::TestEnv;

// Tests from src/cmd/set.rs

// Original: test_set_label
// What it tests: Verifying that set() with a label updates the session's label
// field correctly. Creates a session, sets a label, and verifies the result.
#[test]
#[serial_test::serial]
fn test_set_label() {
    let env = TestEnv::new();
    let orchid_dir = env.dir();
    let config_dir = orchid_dir.clone();
    std::fs::create_dir_all(config_dir.join("sessions")).unwrap();
    let store = Store::with_config_dir(&config_dir).unwrap();
    let meta = store.create(None, None, None).unwrap();
    let result = set(
        meta.id.clone(),
        Some("my-label".to_string()),
        None,
        None,
        &config_dir,
    )
    .unwrap();
    assert_eq!(result["label"], "my-label");
    assert_eq!(result["id"], meta.id);
}

// Original: test_set_updates_metadata
// What it tests: Verifying that set() updates label and working_dir
// metadata fields simultaneously. Creates a session, sets both fields,
// then verifies each field on the Store.
#[test]
#[serial_test::serial]
fn test_set_updates_metadata() {
    let env = TestEnv::new();
    let orchid_dir = env.dir();
    let config_dir = orchid_dir.clone();
    std::fs::create_dir_all(config_dir.join("sessions")).unwrap();
    let store = Store::with_config_dir(&config_dir).unwrap();
    let meta = store.create(None, None, None).unwrap();
    set(
        meta.id.clone(),
        Some("labeled".to_string()),
        Some("/tmp/work".to_string()),
        None,
        &config_dir,
    )
    .unwrap();
    let updated = store.get(&meta.id).unwrap();
    assert_eq!(updated.label.as_deref(), Some("labeled"));
    assert_eq!(updated.working_dir.as_deref(), Some("/tmp/work"));
}
