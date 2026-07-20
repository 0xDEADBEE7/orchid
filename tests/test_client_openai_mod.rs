// Tests for src/client/openai/mod.rs
// These test the OpenAI message conversion and tool schema mapping.

mod support;

use orchid::client::openai::{openai_tool_definitions, to_openai_message, OpenAiListResponse};
use orchid::types::{Message, ToolCall, ToolResult};

#[test]
fn test_to_openai_message_plain() {
    let msg = Message {
        role: "user".to_string(),
        content: "hello".to_string(),
        tool_calls: None,
        tool_result: None,
    };
    let result = to_openai_message(&msg);
    assert_eq!(result.role, "user");
    assert_eq!(result.content, Some("hello".to_string()));
    assert!(result.tool_calls.is_none());
    assert!(result.tool_call_id.is_none());
}

#[test]
fn test_to_openai_message_tool_call() {
    let msg = Message {
        role: "assistant".to_string(),
        content: "".to_string(),
        tool_calls: Some(vec![ToolCall {
            id: "tc1".to_string(),
            name: "bash".to_string(),
            input: serde_json::json!({"cmd": "ls -la"}),
        }]),
        tool_result: None,
    };
    let result = to_openai_message(&msg);
    assert_eq!(result.role, "assistant");
    assert!(result.content.is_none());
    let calls = result.tool_calls.unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].id, "tc1");
    assert_eq!(calls[0].kind, "function");
    assert_eq!(calls[0].function.name, "bash");
    assert!(calls[0].function.arguments.contains("ls -la"));
}

#[test]
fn test_to_openai_message_tool_result() {
    let msg = Message {
        role: "user".to_string(),
        content: "".to_string(),
        tool_calls: None,
        tool_result: Some(ToolResult {
            call_id: "tc1".to_string(),
            content: serde_json::Value::String("output".to_string()),
        }),
    };
    let result = to_openai_message(&msg);
    assert_eq!(result.role, "tool");
    assert_eq!(result.content, Some("output".to_string()));
    assert_eq!(result.tool_call_id, Some("tc1".to_string()));
}

#[test]
fn test_to_openai_message_tool_result_json_object() {
    let msg = Message {
        role: "user".to_string(),
        content: "".to_string(),
        tool_calls: None,
        tool_result: Some(ToolResult {
            call_id: "tc1".to_string(),
            content: serde_json::json!({"file": "foo.rs", "lines": 42}),
        }),
    };
    let result = to_openai_message(&msg);
    assert_eq!(result.role, "tool");
    let content = result.content.unwrap();
    assert!(content.contains("foo.rs"));
}

#[test]
fn test_openai_tool_schema_mapping() {
    let defs = openai_tool_definitions();
    assert_eq!(defs.len(), 3);
    let names: Vec<&str> = defs
        .iter()
        .map(|d| d["function"]["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"bash"));
    assert!(names.contains(&"fs_read"));
    assert!(names.contains(&"fs_edit"));
    for d in &defs {
        assert_eq!(d["type"], "function");
        assert!(d["function"]["parameters"].is_object());
    }
}

#[test]
fn test_openai_response_deserialization() {
    let json = r#"{
        "choices": [{"message": {"content": "Hello world"}}],
        "usage": {"prompt_tokens": 10, "completion_tokens": 20}
    }"#;
    let resp: OpenAiListResponse = serde_json::from_str(json).unwrap();
    assert_eq!(resp.choices.len(), 1);
    assert_eq!(
        resp.choices[0].message.content,
        Some("Hello world".to_string())
    );
    assert_eq!(resp.usage.as_ref().unwrap().prompt_tokens, Some(10));
    assert_eq!(resp.usage.as_ref().unwrap().completion_tokens, Some(20));
}
