mod support;
use orchid::client::anthropic::{to_wire_message, AnthropicResponse};
use orchid::types::{Message, ToolCall, ToolResult};

// Tests from src/client/anthropic/mod.rs
// (test_to_wire_message_plain is already in test_client_anthropic.rs)

// Original: test_to_wire_message_tool_call
// What it tests: Verifying that to_wire_message() correctly converts an assistant
// message with tool_calls into an array of content blocks with type "tool_use",
// preserving the tool name and input.
#[test]
fn test_to_wire_message_tool_call() {
    let m = Message {
        role: "assistant".to_string(),
        content: String::new(),
        tool_calls: Some(vec![ToolCall {
            id: "tc1".to_string(),
            name: "bash".to_string(),
            input: serde_json::json!({"cmd": "ls"}),
        }]),
        tool_result: None,
    };
    let w = to_wire_message(&m);
    assert!(w.content.is_array());
    let arr = w.content.as_array().unwrap();
    assert_eq!(arr[0]["type"], "tool_use");
    assert_eq!(arr[0]["name"], "bash");
}

// Original: test_to_wire_message_tool_result
// What it tests: Verifying that to_wire_message() correctly converts a user message
// with tool_result into an array of content blocks with type "tool_result",
// preserving the tool_use_id and content.
#[test]
fn test_to_wire_message_tool_result() {
    let m = Message {
        role: "user".to_string(),
        content: String::new(),
        tool_calls: None,
        tool_result: Some(ToolResult {
            call_id: "tc1".to_string(),
            content: serde_json::Value::String("output".to_string()),
        }),
    };
    let w = to_wire_message(&m);
    assert!(w.content.is_array());
    let arr = w.content.as_array().unwrap();
    assert_eq!(arr[0]["type"], "tool_result");
    assert_eq!(arr[0]["tool_use_id"], "tc1");
    assert_eq!(arr[0]["content"], "output");
}

// Original: test_to_wire_message_tool_result_json_object
// What it tests: Verifying that when a tool_result contains a JSON object (not
// a plain string), to_wire_message() serializes it as a string containing the
// original JSON structure.
#[test]
fn test_to_wire_message_tool_result_json_object() {
    let m = Message {
        role: "user".to_string(),
        content: String::new(),
        tool_calls: None,
        tool_result: Some(ToolResult {
            call_id: "tc2".to_string(),
            content: serde_json::json!({"foo.rs": "some content"}),
        }),
    };
    let w = to_wire_message(&m);
    let arr = w.content.as_array().unwrap();
    let content_str = arr[0]["content"].as_str().unwrap();
    assert!(content_str.contains("foo.rs"));
}

// Original: test_anthropic_response_deserialization
// What it tests: Verifying that a valid Anthropic API response JSON can be
// deserialized into AnthropicResponse, with content blocks parsed correctly.
#[test]
fn test_anthropic_response_deserialization() {
    let json = r#"{
        "content": [
            {"type": "text", "text": "Hello world"}
        ],
        "stop_reason": "end_turn"
    }"#;
    let response: AnthropicResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.content.len(), 1);
}
