use orchid::log::LogWriter;
use orchid::r#loop::budget::{check, BudgetStatus};
use orchid::types::TokenBudget;
use orchid::types::{MessageEvent, SessionEvent};
mod support;
use std::fs;
use support::TestEnv;

fn setup_convo_with_chars(convo_id: &str, char_count: usize, base: &std::path::Path) {
    let session_dir = base.join("sessions").join(convo_id);
    fs::create_dir_all(&session_dir).unwrap();
    let jsonl = session_dir.join("conversation.jsonl");
    let chunk = "x".repeat(100);
    let mut written = 0usize;
    while written < char_count {
        let event = SessionEvent::Message(MessageEvent::new("user", &chunk));
        LogWriter::append(&jsonl, &event).unwrap();
        written = fs::metadata(&jsonl).unwrap().len() as usize;
    }
}

#[test]
#[serial_test::serial]
fn test_ok() {
    let env = TestEnv::new();
    let base = env.dir();
    let sessions_dir = base.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    setup_convo_with_chars("c1", 30_000, base.as_path());
    let budget = TokenBudget::default();
    assert!(matches!(check("c1", &base, &budget), BudgetStatus::Ok { .. }));
}

#[test]
#[serial_test::serial]
fn test_warning() {
    let env = TestEnv::new();
    let base = env.dir();
    let sessions_dir = base.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    setup_convo_with_chars("c2", 270_000, base.as_path());
    let budget = TokenBudget::default();
    assert!(matches!(check("c2", &base, &budget), BudgetStatus::Warning { .. }));
}

#[test]
#[serial_test::serial]
fn test_exceeded() {
    let env = TestEnv::new();
    let base = env.dir();
    let sessions_dir = base.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    setup_convo_with_chars("c3", 390_000, base.as_path());
    let budget = TokenBudget::default();
    assert!(matches!(
        check("c3", &base, &budget),
        BudgetStatus::Exceeded { .. }
    ));
}
