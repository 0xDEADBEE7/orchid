use orchid::cmd::set;
use orchid::convo::Store;
mod support;
use support::TestEnv;

// Tests from src/cmd/set.rs

// Original: test_set_label
// What it tests: Verifying that set() with a label updates the conversation's label
// field correctly. Creates a conversation, sets a label, and verifies the result.
#[test]
#[serial_test::serial]
fn test_set_label() {
    let env = TestEnv::new();
    let orchid_dir = env.dir();
    let convos_dir = orchid_dir.join("conversations");
    std::fs::create_dir_all(&convos_dir).unwrap();
    let store = Store::with_base(convos_dir);
    let meta = store.create(None, None, None, None, None).unwrap();
    let result = set(meta.id.clone(), Some("my-label".to_string()), None, None, None).unwrap();
    assert_eq!(result["label"], "my-label");
    assert_eq!(result["id"], meta.id);
}

// Original: test_set_updates_metadata
// What it tests: Verifying that set() updates label, persona, and working_dir
// metadata fields simultaneously. Creates a conversation, sets all metadata,
// then verifies each field on the Store.
#[test]
#[serial_test::serial]
fn test_set_updates_metadata() {
    let env = TestEnv::new();
    let orchid_dir = env.dir();
    let convos_dir = orchid_dir.join("conversations");
    std::fs::create_dir_all(&convos_dir).unwrap();
    let store = Store::with_base(convos_dir);
    let meta = store.create(None, None, None, None, None).unwrap();
    set(
        meta.id.clone(),
        Some("labeled".to_string()),
        Some("coder".to_string()),
        Some("/tmp/work".to_string()),
        None,
    )
    .unwrap();
    let updated = store.get(&meta.id).unwrap();
    assert_eq!(updated.label.as_deref(), Some("labeled"));
    assert_eq!(updated.persona.as_deref(), Some("coder"));
    assert_eq!(updated.working_dir.as_deref(), Some("/tmp/work"));
}