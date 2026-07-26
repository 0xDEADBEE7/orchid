use orchid::{config::Settings, model::{Event, Session}, provider::{parse_sse, run, Provider, StreamEvent}};
use std::io;
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

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
fn malformed_sse_is_explicit_and_fails_run() {
    assert!(matches!(
        parse_sse("data: {not-json}\n").as_slice(),
        [StreamEvent::Malformed(_)]
    ));
    let dir = tempfile::tempdir().unwrap();
    orchid::config::init(dir.path()).unwrap();
    let settings = Settings::load(dir.path()).unwrap();
    struct Bad;
    impl Provider for Bad {
        fn reply(&self, _: &str, _: &Session) -> io::Result<String> {
            unreachable!()
        }
        fn stream(&self, _: &str, _: &Session) -> io::Result<Vec<StreamEvent>> {
            Ok(vec![StreamEvent::Malformed("bad response".into())])
        }
    }
    let mut session = Session::new(None, None, None);
    assert!(run(&Bad, &settings, &mut session, "x").is_err());
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
    orchid::config::init(dir.path()).unwrap();
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
    orchid::config::init(dir.path()).unwrap();
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

struct Counted(AtomicUsize);
impl Provider for Counted {
    fn reply(&self, _: &str, _: &Session) -> io::Result<String> {
        unreachable!()
    }
    fn stream(&self, _: &str, _: &Session) -> io::Result<Vec<StreamEvent>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(vec![StreamEvent::Text("should not arrive".into())])
    }
}

#[test]
fn token_threshold_stops_before_provider_request() {
    let dir = tempfile::tempdir().unwrap();
    orchid::config::init(dir.path()).unwrap();
    std::fs::write(
        dir.path().join("policies/default.json"),
        r#"{"max_tokens":1}"#,
    )
    .unwrap();
    let settings = Settings::load(dir.path()).unwrap();
    let provider = Counted(AtomicUsize::new(0));
    let mut session = Session::new(None, None, None);
    let error = run(&provider, &settings, &mut session, "a long pending request").unwrap_err();
    assert!(error.to_string().contains("token threshold"));
    assert_eq!(provider.0.load(Ordering::SeqCst), 0);
    assert_eq!(session.state.token_estimate, 0);
    assert!(session.state.termination_reason.is_some());
}

#[test]
fn token_estimate_is_serialized_request_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    orchid::config::init(dir.path()).unwrap();
    let settings = Settings::load(dir.path()).unwrap();
    struct NoUsage;
    impl Provider for NoUsage {
        fn reply(&self, _: &str, _: &Session) -> io::Result<String> {
            Ok("done".into())
        }
    }
    let mut session = Session::new(None, None, None);
    session.append(Session::message("user", "123456789".into()));
    run(&NoUsage, &settings, &mut session, "").unwrap();
    let expected = serde_json::to_string(&vec![
        orchid::client::Message {
            role: "user".into(),
            content: "123456789".into(),
            tool_calls: Vec::new(),
            tool_result: None,
        },
        orchid::client::Message {
            role: "user".into(),
            content: "".into(),
            tool_calls: Vec::new(),
            tool_result: None,
        },
    ])
    .unwrap()
    .len() as u32
        / 3;
    assert_eq!(session.state.token_estimate, expected);
}

#[test]
fn tool_failure_persists_correlated_error_result() {
    let dir = tempfile::tempdir().unwrap();
    orchid::config::init(dir.path()).unwrap();
    let settings = Settings::load(dir.path()).unwrap();
    struct FailingTool;
    impl Provider for FailingTool {
        fn reply(&self, _: &str, _: &Session) -> io::Result<String> {
            unreachable!()
        }
        fn stream(&self, _: &str, _: &Session) -> io::Result<Vec<StreamEvent>> {
            Ok(vec![StreamEvent::ToolCall {
                name: "bash".into(),
                input: serde_json::json!({"cmd":"false"}),
            }])
        }
    }
    let mut session = Session::new(None, None, None);
    assert!(run(&FailingTool, &settings, &mut session, "run").is_err());
    let call_id = session
        .events
        .iter()
        .find_map(|event| match event {
            Event::ToolCall { calls, .. } => calls.first().map(|call| call.call_id.clone()),
            _ => None,
        })
        .unwrap();
    assert!(session.events.iter().any(|event| matches!(event, Event::ToolResult { call_id: id, content, .. } if id == &call_id && content.get("error").is_some())));
}
