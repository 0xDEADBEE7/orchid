use orchid::cmd::{delete, list};
use orchid::SessionStore;
use orchid::SessionStore as Store;
mod support;
use support::TestEnv;

#[test]
fn test_list_uses_selected_config_sessions() {
    let env = TestEnv::new();
    let selected = env.dir().join("selected");
    let other = env.dir().join("other");
    let selected_store = Store::with_config_dir(&selected).unwrap();
    let other_store = Store::with_config_dir(&other).unwrap();
    let selected_meta = selected_store
        .create(Some("selected".to_string()), None, None, None, None)
        .unwrap();
    other_store
        .create(Some("other".to_string()), None, None, None, None)
        .unwrap();

    let result = list(&selected, None).unwrap();
    let items = result.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], selected_meta.id);
}

// Original: test_delete_not_found
// What it tests: Verifying that deleting a non-existent conversation (fake 32-char ID)
// returns an error containing "not found" or "session not found".
#[test]
fn test_delete_not_found() {
    let env = TestEnv::new();
    let fake_id = "a".repeat(32);
    let err = delete(fake_id, &env.dir()).unwrap_err();
    assert!(
        err.contains("not found") || err.contains("session not found"),
        "got: {}",
        err
    );
}

// Original: test_delete_creates_archive
// What it tests: Verifying that deleting an existing conversation archives it:
// the session directory is removed, and the conversation appears in .archive.
// Uses serial_test to avoid race conditions.
#[test]
#[serial_test::serial]
fn test_delete_creates_archive() {
    let env = TestEnv::new();
    let orchid_dir = env.dir();
    let sessions_dir = orchid_dir.join("sessions");
    let store = Store::with_base(sessions_dir.clone());
    let meta = store.create(None, None, None, None, None).unwrap();
    assert!(sessions_dir.join(&meta.id).exists());
    let result = delete(meta.id.clone(), &orchid_dir).unwrap();
    assert_eq!(result["id"], meta.id);
    assert_eq!(result["status"], "archived");
    assert!(
        !sessions_dir.join(&meta.id).exists(),
        "session directory should be gone after archive"
    );
    assert!(
        sessions_dir.join(".archive").join(&meta.id).exists(),
        "conversation should be in .archive"
    );
}
