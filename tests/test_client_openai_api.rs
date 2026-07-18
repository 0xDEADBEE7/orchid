// Tests for src/client/openai/api.rs
// These test the OpenAI API client's request body building logic.

mod support;

use orchid::client::openai::OpenAiClient;
use orchid::types::Message;

#[test]
fn test_build_request_body_with_system() {
    std::env::set_var("OPENAI_API_KEY", "test-key");
    let client = OpenAiClient::new().unwrap();
    let api = client.api_client();

    let messages = vec![Message {
        role: "user".to_string(),
        content: "Hello".to_string(),
        tool_calls: None,
        tool_result: None,
    }];

    let body = api
        .build_request_body(
            "You are a helpful assistant".to_string(),
            messages,
            false,
        )
        .unwrap();
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();

    // System message should be first
    let messages = json["messages"].as_array().unwrap();
    assert_eq!(messages[0]["role"], "system");
    assert_eq!(messages[0]["content"], "You are a helpful assistant");
    // User message should be second
    assert_eq!(messages[1]["role"], "user");
    assert_eq!(messages[1]["content"], "Hello");
    assert_eq!(json["messages"].as_array().unwrap().len(), 2);
}

#[test]
fn test_build_request_body_streaming() {
    std::env::set_var("OPENAI_API_KEY", "test-key");
    let client = OpenAiClient::new().unwrap();
    let api = client.api_client();

    let messages = vec![Message {
        role: "user".to_string(),
        content: "Hello".to_string(),
        tool_calls: None,
        tool_result: None,
    }];

    // Non-streaming: no "stream" key
    let body = api
        .build_request_body(String::new(), messages.clone(), false)
        .unwrap();
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(json.as_object().unwrap().get("stream").is_none());

    // Streaming: stream = true
    let body = api
        .build_request_body(String::new(), messages, true)
        .unwrap();
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["stream"], true);
}
