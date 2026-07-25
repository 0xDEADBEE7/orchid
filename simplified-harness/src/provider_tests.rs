use super::*;
use std::cell::Cell;

#[test]
fn parses_provider_sse_text_and_done() {
    let events =
        parse_sse("data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\ndata: [DONE]\n");
    assert_eq!(
        events,
        vec![StreamEvent::Text("hi".into()), StreamEvent::Done]
    );
}

#[test]
fn assembles_multiple_fragmented_tool_calls() {
    let first = serde_json::json!({"choices":[{"delta":{"tool_calls":[
        {"index":0,"function":{"name":"bash","arguments":"{\"cmd\":\"ls\""}},
        {"index":1,"function":{"name":"fs_read","arguments":"{\"paths\":[\"a\"]"}}
    ]}}]})
    .to_string();
    let second = serde_json::json!({"choices":[{"delta":{"tool_calls":[
        {"index":0,"function":{"arguments":"}"}}, {"index":1,"function":{"arguments":"}"}}
    ]}}]})
    .to_string();
    let events = parse_sse(&format!("data: {first}\ndata: {second}\ndata: [DONE]\n"));
    assert!(matches!(events[0], StreamEvent::ToolCall { ref name, .. } if name == "bash"));
    assert!(matches!(events[1], StreamEvent::ToolCall { ref name, .. } if name == "fs_read"));
}

struct Chunks;
impl Provider for Chunks {
    fn reply(&self, _: &str, _: &Session) -> io::Result<String> {
        unreachable!()
    }
    fn stream(&self, _: &str, _: &Session) -> io::Result<Vec<StreamEvent>> {
        Ok(vec![
            StreamEvent::Text("a".into()),
            StreamEvent::Text("b".into()),
            StreamEvent::Done,
        ])
    }
}

#[test]
fn run_records_reassembled_stream() {
    let dir = tempfile::tempdir().unwrap();
    crate::config::init(dir.path()).unwrap();
    let settings = Settings::load(dir.path()).unwrap();
    let mut session = Session::new(None, None, None);
    assert_eq!(run(&Chunks, &settings, &mut session, "x").unwrap(), "ab");
    assert!(
        matches!(session.events.last(), Some(Event::Message { content, .. }) if content == "ab")
    );
}

struct ToolThenText(Cell<bool>);
impl Provider for ToolThenText {
    fn reply(&self, _: &str, _: &Session) -> io::Result<String> {
        unreachable!()
    }
    fn stream(&self, _: &str, _: &Session) -> io::Result<Vec<StreamEvent>> {
        if self.0.replace(true) {
            Ok(vec![StreamEvent::Text("done".into())])
        } else {
            Ok(vec![StreamEvent::ToolCall {
                name: "fs_read".into(),
                input: serde_json::json!({"paths":["config.json"]}),
            }])
        }
    }
}

#[test]
fn tool_results_are_fed_back_until_text_arrives() {
    let dir = tempfile::tempdir().unwrap();
    crate::config::init(dir.path()).unwrap();
    std::fs::write(
        dir.path().join("policies/default.json"),
        r#"{"tools":["fs_read"]}"#,
    )
    .unwrap();
    let settings = Settings::load(dir.path()).unwrap();
    let mut session = Session::new(None, None, None);
    let answer = run(
        &ToolThenText(Cell::new(false)),
        &settings,
        &mut session,
        "read",
    )
    .unwrap();
    assert_eq!(answer, "done");
    assert!(session
        .events
        .iter()
        .any(|e| matches!(e, Event::ToolResult { .. })));
}
