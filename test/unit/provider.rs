use orchid::{
    config::Settings,
    model::{Event, Session},
    provider::{run, Provider, StreamEvent},
};
use std::cell::Cell;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Chunks;
impl Provider for Chunks {
    fn stream(&self, _: &Session) -> io::Result<Vec<StreamEvent>> {
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
    assert_eq!(run(&Chunks, &settings, &mut session).unwrap(), "ab");
    assert!(
        matches!(session.events.last(), Some(Event::Message { content, .. }) if content == "ab")
    );
}

struct ToolThenText(Cell<bool>);
impl Provider for ToolThenText {
    fn stream(&self, _: &Session) -> io::Result<Vec<StreamEvent>> {
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
        dir.path().join("agents/default.json"),
        r#"{"prompt":"default","tools":["fs_read"]}"#,
    )
    .unwrap();
    let settings = Settings::load(dir.path()).unwrap();
    let mut session = Session::new(None, None, None);
    let answer = run(&ToolThenText(Cell::new(false)), &settings, &mut session).unwrap();
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
    fn stream(&self, _: &Session) -> io::Result<Vec<StreamEvent>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(vec![StreamEvent::Text("should not arrive".into())])
    }
}

#[test]
fn token_threshold_stops_before_provider_request() {
    let dir = tempfile::tempdir().unwrap();
    orchid::config::init(dir.path()).unwrap();
    std::fs::write(
        dir.path().join("agents/default.json"),
        r#"{"prompt":"default","max_tokens":1}"#,
    )
    .unwrap();
    let settings = Settings::load(dir.path()).unwrap();
    let provider = Counted(AtomicUsize::new(0));
    let mut session = Session::new(None, None, None);
    session.metadata.token_estimate = 2;
    let error = run(&provider, &settings, &mut session).unwrap_err();
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
        dir.path().join("agents/default.json"),
        r#"{"prompt":"default","max_tokens":120000}"#,
    )
    .unwrap();
    let settings = Settings::load(dir.path()).unwrap();
    let mut session = Session::new(None, None, None);
    session.metadata.token_estimate = 58_441;
    session.metadata.token_usage.marginal_input = 58_441;
    session.metadata.token_usage.cumulative_input = 1_134_441;
    struct Reported;
    impl Provider for Reported {
        fn stream(&self, _: &Session) -> io::Result<Vec<StreamEvent>> {
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
    let answer = run(&Reported, &settings, &mut session).unwrap();
    assert_eq!(answer, "allowed");
    assert_eq!(session.metadata.token_estimate, 58_441);
}

#[test]
fn negative_one_disables_token_threshold() {
    let dir = tempfile::tempdir().unwrap();
    orchid::config::init(dir.path()).unwrap();
    std::fs::write(
        dir.path().join("agents/default.json"),
        r#"{"prompt":"default","max_tokens":-1}"#,
    )
    .unwrap();
    let settings = Settings::load(dir.path()).unwrap();
    let mut session = Session::new(None, None, None);
    session.metadata.token_estimate = u32::MAX;
    assert_eq!(
        run(&Counted(AtomicUsize::new(0)), &settings, &mut session).unwrap(),
        "should not arrive"
    );
    assert!(session.metadata.token_estimate > 0);
}

#[test]
fn absent_token_limit_does_not_add_a_fallback_limit() {
    assert_eq!(orchid::agent::Agent::default().max_tokens, None);
}

#[test]
fn token_estimate_falls_back_to_a_byte_count() {
    let dir = tempfile::tempdir().unwrap();
    orchid::config::init(dir.path()).unwrap();
    let settings = Settings::load(dir.path()).unwrap();
    struct NoUsage;
    impl Provider for NoUsage {
        fn stream(&self, _: &Session) -> io::Result<Vec<StreamEvent>> {
            Ok(vec![StreamEvent::Text("done".into())])
        }
    }
    let mut session = Session::new(None, None, None);
    session.append(Session::message("user", "123456789".into()));
    run(&NoUsage, &settings, &mut session).unwrap();
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
        fn stream(&self, _: &Session) -> io::Result<Vec<StreamEvent>> {
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
    assert_eq!(run(&Reported, &settings, &mut session).unwrap(), "done");
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
    // The fake model calls a failing tool forever; the token cap ends the loop.
    std::fs::write(
        dir.path().join("agents/default.json"),
        r#"{"prompt":"default","max_tokens":300}"#,
    )
    .unwrap();
    let settings = Settings::load(dir.path()).unwrap();
    struct FailingTool;
    impl Provider for FailingTool {
        fn stream(&self, _: &Session) -> io::Result<Vec<StreamEvent>> {
            Ok(vec![StreamEvent::ToolCall {
                name: "bash".into(),
                input: serde_json::json!({"cmd":"false"}),
            }])
        }
    }
    let mut session = Session::new(None, None, None);
    assert!(run(&FailingTool, &settings, &mut session).is_err());
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

#[test]
fn text_with_tool_calls_runs_the_calls_and_bad_arguments_become_tool_errors() {
    let dir = tempfile::tempdir().unwrap();
    orchid::config::init(dir.path()).unwrap();
    std::fs::write(
        dir.path().join("agents/default.json"),
        r#"{"prompt":"default","tools":["fs_read"]}"#,
    )
    .unwrap();
    let settings = Settings::load(dir.path()).unwrap();
    struct Preamble(Cell<bool>);
    impl Provider for Preamble {
        fn stream(&self, _: &Session) -> io::Result<Vec<StreamEvent>> {
            if self.0.replace(true) {
                return Ok(vec![StreamEvent::Text("done".into())]);
            }
            Ok(vec![
                StreamEvent::Text("Let me check.".into()),
                StreamEvent::ToolCall {
                    name: "fs_read".into(),
                    input: "{bad".into(),
                },
            ])
        }
    }
    let mut session = Session::new(None, None, None);
    let answer = run(&Preamble(Cell::new(false)), &settings, &mut session).unwrap();
    assert_eq!(answer, "done");
    assert!(session
        .events
        .iter()
        .any(|e| matches!(e, Event::Message { content, .. } if content == "Let me check.")));
    assert!(session
        .events
        .iter()
        .any(|e| matches!(e, Event::ToolResult { content, .. }
        if content["error"].as_str().unwrap().contains("JSON object"))));
}

#[test]
fn oversized_tool_output_keeps_head_and_tail() {
    let big = format!("{}{}", "a".repeat(60_000), "z".repeat(10));
    let clipped = orchid::tools::clip(serde_json::Value::String(big));
    let text = clipped.as_str().unwrap();
    assert!(text.len() < 50_100 && text.starts_with('a') && text.ends_with("zzzzzzzzzz"));
    assert!(text.contains("characters truncated"));
    let small = serde_json::json!({"ok": true});
    assert_eq!(orchid::tools::clip(small.clone()), small);
}

#[test]
fn transient_client_errors_are_retried_and_others_are_not() {
    use orchid::client::{ClientError, ClientErrorKind};
    let dir = tempfile::tempdir().unwrap();
    orchid::config::init(dir.path()).unwrap();
    let settings = Settings::load(dir.path()).unwrap();
    struct Flaky(AtomicUsize, ClientErrorKind);
    impl Provider for Flaky {
        fn stream(&self, _: &Session) -> io::Result<Vec<StreamEvent>> {
            if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
                return Err(io::Error::other(ClientError::new(
                    self.1,
                    "HTTP status 503",
                )));
            }
            Ok(vec![StreamEvent::Text("recovered".into())])
        }
    }
    let mut session = Session::new(None, None, None);
    let flaky = Flaky(AtomicUsize::new(0), ClientErrorKind::Unavailable);
    assert_eq!(run(&flaky, &settings, &mut session).unwrap(), "recovered");
    assert_eq!(flaky.0.load(Ordering::SeqCst), 2);
    let denied = Flaky(AtomicUsize::new(0), ClientErrorKind::Authentication);
    assert!(run(&denied, &settings, &mut Session::new(None, None, None)).is_err());
    assert_eq!(denied.0.load(Ordering::SeqCst), 1);
}
