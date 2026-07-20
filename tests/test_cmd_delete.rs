use orchid::cmd::delete;
use orchid::convo::Store;
mod support;
use support::TestEnv;

// Tests from src/cmd/delete.rs

// Original: test_delete_not_found
// What it tests: Verifying that deleting a non-existent conversation (fake 32-char ID)
// returns an error containing "not found" or "conversation not found".
#[test]
fn test_delete_not_found() {
    let _env = TestEnv::new();
    let fake_id = "a".repeat(32);
    let err = delete(fake_id).unwrap_err();
    assert!(
        err.contains("not found") || err.contains("conversation not found"),
        "got: {}",
        err
    );
}

// Original: test_delete_creates_archive
// What it tests: Verifying that deleting an existing conversation archives it:
// the conversation dir is removed, and the conversation appears in .archive.
// Uses serial_test to avoid race conditions.
#[test]
#[serial_test::serial]
fn test_delete_creates_archive() {
    let env = TestEnv::new();
    let orchid_dir = env.dir();
    let convos_dir = orchid_dir.join("conversations");
    std::fs::create_dir_all(&convos_dir).unwrap();
    let store = Store::with_base(convos_dir.clone());
    let meta = store.create(None, None, None, None, None).unwrap();
    assert!(convos_dir.join(&meta.id).exists());
    let result = delete(meta.id.clone()).unwrap();
    assert_eq!(result["id"], meta.id);
    assert_eq!(result["status"], "archived");
    assert!(
        !convos_dir.join(&meta.id).exists(),
        "conversation dir should be gone after archive"
    );
    assert!(
        convos_dir.join(".archive").join(&meta.id).exists(),
        "conversation should be in .archive"
    );
}
