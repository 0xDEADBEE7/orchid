use orchid::r#loop::history::build_message_history;
use orchid::log::{DiagLogger, LogLevel, LogReader, LogWriter};
use orchid::types::{
    ConvoEvent, MessageEvent, ToolCall, ToolCallEvent, ToolResult, ToolResultEvent, Message,
};
mod support;
use support::TestEnv;
use std::fs;
use tempfile::TempDir;

fn build_message_history_from_path(path: &std::path::Path) -> Result<Vec<Message>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let all_events = LogReader::read_lines(path)?;
    let mut messages = Vec::new();
    for event in all_events {
        match event {
            ConvoEvent::Message(e) => {
                if e.message.role != "system" {
                    messages.push(Message {
                        role: e.message.role,
                        content: e.message.content,
                        tool_calls: None,
                        tool_result: None,
                    });
                }
            }
            ConvoEvent::ToolCall(e) => {
                messages.push(Message {
                    role: "assistant".to_string(),
                    content: String::new(),
                    tool_calls: Some(e.tool_call.calls),
                    tool_result: None,
                });
            }
            ConvoEvent::ToolResult(e) => {
                messages.push(Message {
                    role: "user".to_string(),
                    content: String::new(),
                    tool_calls: None,
                    tool_result: Some(e.tool_result),
                });
            }
            ConvoEvent::Reasoning(_) => {}
        }
    }
    Ok(messages)
}

#[test]
fn test_build_empty_history() {
    let result = build_message_history("nonexistent-id", &DiagLogger::noop());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 0);
}

#[test]
fn test_build_message_history() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let convo_path = temp_dir.path().join("test-id");
    fs::create_dir(&convo_path)?;
    let jsonl_path = convo_path.join("conversation.jsonl");
    LogWriter::append(
        &jsonl_path,
        &ConvoEvent::Message(MessageEvent::new("user", "hello")),
    )?;
    LogWriter::append(
        &jsonl_path,
        &ConvoEvent::Message(MessageEvent::new("assistant", "hi there")),
    )?;
    let messages = build_message_history_from_path(&jsonl_path)?;
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[1].role, "assistant");
    Ok(())
}

#[test]
#[serial_test::serial]
fn test_stale_read_replacement() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::new();
    let base = env.dir();
    let convos_dir = base.join("conversations");
    std::fs::create_dir_all(&convos_dir).unwrap();
    let convo_id = "stale-test-001";
    let convo_path = convos_dir.join(convo_id);
    fs::create_dir_all(&convo_path)?;
    let jsonl_path = convo_path.join("conversation.jsonl");
    let tc1 = ConvoEvent::ToolCall(ToolCallEvent::new(vec![ToolCall {
        id: "c1".to_string(),
        name: "fs_read".to_string(),
        input: serde_json::json!({"paths": ["foo.rs"]}),
    }]));
    let tr1 = ConvoEvent::ToolResult(ToolResultEvent::new(ToolResult {
        call_id: "c1".to_string(),
        content: serde_json::json!({"foo.rs": "original content"}),
    }));
    let tc2 = ConvoEvent::ToolCall(ToolCallEvent::new(vec![ToolCall {
        id: "c2".to_string(),
        name: "fs_read".to_string(),
        input: serde_json::json!({"paths": ["foo.rs"]}),
    }]));
    let tr2 = ConvoEvent::ToolResult(ToolResultEvent::new(ToolResult {
        call_id: "c2".to_string(),
        content: serde_json::json!({"foo.rs": "updated content"}),
    }));
    for e in [&tc1, &tr1, &tc2, &tr2] {
        LogWriter::append(&jsonl_path, e)?;
    }
    let disk_before = fs::read_to_string(&jsonl_path)?;
    let log = DiagLogger::for_convo(convo_path.clone(), LogLevel::Debug);
    let messages = build_message_history(convo_id, &log)?;
    let user_messages: Vec<&Message> = messages.iter().filter(|m| m.role == "user").collect();
    assert_eq!(user_messages.len(), 2);
    let tr1_content = &user_messages[0].tool_result.as_ref().unwrap().content;
    assert_eq!(
        tr1_content["foo.rs"],
        serde_json::json!({"stale": true}),
        "expected stale marker in-memory, got: {}",
        tr1_content
    );
    let tr2_content = &user_messages[1].tool_result.as_ref().unwrap().content;
    assert_eq!(
        tr2_content["foo.rs"].as_str().unwrap(),
        "updated content",
        "expected updated content in-memory, got: {}",
        tr2_content
    );
    let disk_after = fs::read_to_string(&jsonl_path)?;
    assert_eq!(
        disk_before, disk_after,
        "build_message_history must not rewrite the JSONL"
    );
    let log_path = convo_path.join("orchid.log");
    let log_contents = fs::read_to_string(&log_path)?;
    assert!(
        log_contents.contains("tombstone_savings"),
        "expected tombstone_savings in orchid.log"
    );
    assert!(
        log_contents.contains("tombstones=1"),
        "expected tombstones=1 in log"
    );
    Ok(())
}
