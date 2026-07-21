use orchid::client::anthropic::to_wire_message;
use orchid::types::Message;

mod support;

#[test]
fn test_to_wire_message_plain() {
    let msg = Message {
        role: "user".to_string(),
        content: "hello".to_string(),
        tool_calls: None,
        tool_result: None,
    };
    let wire = to_wire_message(&msg);
    assert_eq!(wire.role, "user");
}
