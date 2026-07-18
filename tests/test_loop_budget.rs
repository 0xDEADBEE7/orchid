use orchid::types::TokenBudget;
use orchid::r#loop::budget::{check, BudgetStatus};
use orchid::log::LogWriter;
use orchid::types::{ConvoEvent, MessageEvent};
mod support;
use support::TestEnv;
use std::fs;

fn setup_convo_with_chars(convo_id: &str, char_count: usize, base: &std::path::Path) {
    let convo_dir = base.join("conversations").join(convo_id);
    fs::create_dir_all(&convo_dir).unwrap();
    let jsonl = convo_dir.join("conversation.jsonl");
    let chunk = "x".repeat(100);
    let mut written = 0usize;
    while written < char_count {
        let event = ConvoEvent::Message(MessageEvent::new("user", &chunk));
        LogWriter::append(&jsonl, &event).unwrap();
        written = fs::metadata(&jsonl).unwrap().len() as usize;
    }
}

#[test]
#[serial_test::serial]
fn test_ok() {
    let env = TestEnv::new();
    let base = env.dir();
    let convos_dir = base.join("conversations");
    std::fs::create_dir_all(&convos_dir).unwrap();
    setup_convo_with_chars("c1", 30_000, base.as_path());
    let budget = TokenBudget::default();
    assert!(matches!(check("c1", &budget), BudgetStatus::Ok { .. }));
}

#[test]
#[serial_test::serial]
fn test_warning() {
    let env = TestEnv::new();
    let base = env.dir();
    let convos_dir = base.join("conversations");
    std::fs::create_dir_all(&convos_dir).unwrap();
    setup_convo_with_chars("c2", 270_000, base.as_path());
    let budget = TokenBudget::default();
    assert!(matches!(check("c2", &budget), BudgetStatus::Warning { .. }));
}

#[test]
#[serial_test::serial]
fn test_exceeded() {
    let env = TestEnv::new();
    let base = env.dir();
    let convos_dir = base.join("conversations");
    std::fs::create_dir_all(&convos_dir).unwrap();
    setup_convo_with_chars("c3", 390_000, base.as_path());
    let budget = TokenBudget::default();
    assert!(matches!(
        check("c3", &budget),
        BudgetStatus::Exceeded { .. }
    ));
}
