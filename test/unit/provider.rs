use orchid::{
    config::Settings,
    model::{Event, Session},
    provider::{parse_sse, run, Provider, StreamEvent},
};
use std::cell::Cell;
use std::io;
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
    assert_eq!(session.metadata.token_usage.requests, 2);
    assert!(
        session.metadata.token_usage.cumulative_input
            > u64::from(session.metadata.token_usage.marginal_input)
    );
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
    session.metadata.token_estimate = 2;
    let error = run(&provider, &settings, &mut session, "a long pending request").unwrap_err();
    assert_eq!(error.to_string(), "token threshold exceeded: 2 > 1");
    assert_eq!(provider.0.load(Ordering::SeqCst), 0);
    assert_eq!(session.metadata.token_estimate, 2);
    assert!(session.metadata.termination_reason.is_some());
}

#[test]
fn cumulative_usage_does_not_count_toward_the_token_limit() {
    let dir = tempfile::tempdir().unwrap();
    orchid::config::init(dir.path()).unwrap();
    std::fs::write(
        dir.path().join("policies/default.json"),
        r#"{"max_tokens":120000}"#,
    )
    .unwrap();
    let settings = Settings::load(dir.path()).unwrap();
    let mut session = Session::new(None, None, None);
    session.metadata.token_estimate = 58_441;
    session.metadata.token_usage.marginal_input = 58_441;
    session.metadata.token_usage.cumulative_input = 1_134_441;
    struct Reported;
    impl Provider for Reported {
        fn reply(&self, _: &str, _: &Session) -> io::Result<String> {
            unreachable!()
        }
        fn stream(&self, _: &str, _: &Session) -> io::Result<Vec<StreamEvent>> {
            Ok(vec![
                StreamEvent::Usage(orchid::model::Usage {
                    input: 58_441,
                    output: 1,
                    cached_input: 0,
                }),
                StreamEvent::Text("allowed".into()),
            ])
        }
    }
    let answer = run(&Reported, &settings, &mut session, "continue").unwrap();
    assert_eq!(answer, "allowed");
    assert_eq!(session.metadata.token_estimate, 58_441);
}

#[test]
fn negative_one_disables_token_threshold() {
    let dir = tempfile::tempdir().unwrap();
    orchid::config::init(dir.path()).unwrap();
    std::fs::write(
        dir.path().join("policies/default.json"),
        r#"{"max_tokens":-1}"#,
    )
    .unwrap();
    let settings = Settings::load(dir.path()).unwrap();
    let mut session = Session::new(None, None, None);
    session.metadata.token_estimate = u32::MAX;
    assert_eq!(
        run(
            &Counted(AtomicUsize::new(0)),
            &settings,
            &mut session,
            "a long pending request"
        )
        .unwrap(),
        "should not arrive"
    );
    assert!(session.metadata.token_estimate > 0);
}

#[test]
fn absent_token_limit_does_not_add_a_fallback_limit() {
    assert_eq!(orchid::config::Policy::default().max_tokens(), None);
}

#[test]
fn token_estimate_uses_o200k_jsonl_tokens() {
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
    assert!(session.metadata.token_estimate > 0);
    assert_eq!(
        session.metadata.token_usage.marginal_input,
        session.metadata.token_estimate
    );
    assert_eq!(
        session.metadata.token_usage.cumulative_input,
        u64::from(session.metadata.token_estimate)
    );
    assert_eq!(session.metadata.token_usage.requests, 1);
    assert_eq!(session.metadata.token_usage.method, "local_tokenizer");
}

#[test]
fn provider_reported_usage_beats_local_estimate_and_tracks_cached_input() {
    let dir = tempfile::tempdir().unwrap();
    orchid::config::init(dir.path()).unwrap();
    let settings = Settings::load(dir.path()).unwrap();
    struct Reported;
    impl Provider for Reported {
        fn reply(&self, _: &str, _: &Session) -> io::Result<String> {
            unreachable!()
        }
        fn stream(&self, _: &str, _: &Session) -> io::Result<Vec<StreamEvent>> {
            Ok(vec![
                StreamEvent::Usage(orchid::model::Usage {
                    input: 120,
                    output: 30,
                    cached_input: 80,
                }),
                StreamEvent::Text("done".into()),
            ])
        }
    }
    let mut session = Session::new(None, None, None);
    assert_eq!(
        run(&Reported, &settings, &mut session, "x").unwrap(),
        "done"
    );
    assert_eq!(session.metadata.token_usage.marginal_input, 120);
    assert_eq!(session.metadata.token_estimate, 120);
    assert_eq!(session.metadata.token_usage.cumulative_input, 120);
    assert_eq!(session.metadata.token_usage.cumulative_output, 30);
    assert_eq!(session.metadata.token_usage.cumulative_cached_input, 80);
    assert_eq!(session.metadata.token_usage.method, "provider_reported");
    assert!(!session
        .events
        .iter()
        .any(|event| matches!(event, Event::Usage { .. })));
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
