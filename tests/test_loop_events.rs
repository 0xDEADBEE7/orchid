use orchid::get_convo_jsonl_path;

mod support;

// Test from src/loop/events.rs
// Original: test_get_convo_jsonl_path
// What it tests: Verifying that get_convo_jsonl_path returns a valid path containing
// the conversation ID and 'conversation.jsonl'. This is a simple path construction test.
#[test]
fn test_get_convo_jsonl_path() {
    let path = get_convo_jsonl_path("test-id");
    assert!(path.is_ok());
    let p = path.unwrap();
    assert!(p.to_string_lossy().contains("test-id"));
    assert!(p.to_string_lossy().contains("conversation.jsonl"));
}