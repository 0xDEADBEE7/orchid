use orchid::log::{LogReader, LogWriter};
use orchid::types::{MessageEvent, SessionEvent};

mod support;

// Original: test_append_and_read
// What it tests: LogWriter::append writes SessionEvent as JSONL to a file,
// and LogReader::read_lines reads them back, verifying round-trip serialization
// preserves event data (role and content) correctly.
#[test]
fn test_append_and_read() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = tempfile::TempDir::new()?;
    let log_path = tmp_dir.path().join("test.jsonl");

    let e1 = SessionEvent::Message(MessageEvent::new("user", "hello"));
    let e2 = SessionEvent::Message(MessageEvent::new("assistant", "hi there"));

    LogWriter::append(&log_path, &e1)?;
    LogWriter::append(&log_path, &e2)?;

    let events = LogReader::read_lines(&log_path)?;

    assert_eq!(events.len(), 2);
    match &events[0] {
        SessionEvent::Message(e) => {
            assert_eq!(e.message.role, "user");
            assert_eq!(e.message.content, "hello");
        }
        _ => panic!("expected Message event"),
    }
    match &events[1] {
        SessionEvent::Message(e) => {
            assert_eq!(e.message.role, "assistant");
            assert_eq!(e.message.content, "hi there");
        }
        _ => panic!("expected Message event"),
    }

    Ok(())
}
