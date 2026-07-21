use orchid::get_session_jsonl_path;

mod support;

// Test from src/loop/events.rs
// Original: test_get_convo_jsonl_path
// What it tests: Verifying that get_convo_jsonl_path returns a valid path containing
// the session ID and 'conversation.jsonl'. This is a simple path construction test.
#[test]
fn test_get_convo_jsonl_path() {
    let path = get_session_jsonl_path("test-id");
    assert!(path.is_ok());
    let p = path.unwrap();
    assert!(p.to_string_lossy().contains("test-id"));
    assert!(p.to_string_lossy().contains("conversation.jsonl"));
}

#[test]
fn event_appends_use_the_selected_config_directory() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let store = orchid::SessionStore::with_config_dir(first.path()).unwrap();
    let session = store.create(None, None, None).unwrap();
    std::fs::create_dir_all(second.path().join("sessions").join(&session.id)).unwrap();

    orchid::r#loop::events::append_system(&session.id, second.path(), "selected").unwrap();

    let selected = std::fs::read_to_string(
        second
            .path()
            .join("sessions")
            .join(&session.id)
            .join("conversation.jsonl"),
    )
    .unwrap();
    assert!(selected.contains("selected"));
    assert!(!first
        .path()
        .join("sessions")
        .join(&session.id)
        .join("conversation.jsonl")
        .exists());
}
