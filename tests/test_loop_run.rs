mod support;

use orchid::config::resolve::EffectiveSessionConfig;
use orchid::config::{HookConfiguration, Permissions, PolicyLimits};
use orchid::provider::{Provider, ProviderError, Response, StreamEvent};
use orchid::r#loop::{build_context, build_context_with_budget, run_loop};
use orchid::types::{Message, TokenBudget, ToolCall};
use orchid::{SessionStore, Status};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

struct SequenceProvider {
    streams: Mutex<Vec<Result<Vec<Result<StreamEvent, ProviderError>>, ProviderError>>>,
    calls: Arc<Mutex<usize>>,
}

impl SequenceProvider {
    fn new(streams: Vec<Result<Vec<Result<StreamEvent, ProviderError>>, ProviderError>>) -> Self {
        Self {
            streams: Mutex::new(streams),
            calls: Arc::new(Mutex::new(0)),
        }
    }

    fn calls(&self) -> usize {
        *self.calls.lock().unwrap()
    }
}

impl Provider for SequenceProvider {
    fn send(&self, _system: String, _messages: Vec<Message>) -> Result<Response, ProviderError> {
        Err(ProviderError::InvalidResponse(
            "non-streaming path used".into(),
        ))
    }

    fn send_streaming(
        &self,
        _system: String,
        _messages: Vec<Message>,
    ) -> Result<Box<dyn Iterator<Item = Result<StreamEvent, ProviderError>>>, ProviderError> {
        *self.calls.lock().unwrap() += 1;
        let stream = self.streams.lock().unwrap().remove(0)?;
        Ok(Box::new(stream.into_iter()))
    }
}

fn effective(working_dir: PathBuf) -> EffectiveSessionConfig {
    EffectiveSessionConfig {
        policy_name: "test".into(),
        policy_hash: "test-hash".into(),
        prompt_name: None,
        connection_candidates: vec![],
        prompt: "test prompt".into(),
        working_dir,
        permissions: Permissions {
            tools: vec![],
            paths: vec![],
        },
        limits: PolicyLimits {
            token_warn_threshold: None,
            token_hard_limit: None,
            max_steps: None,
        },
        env_vars: HashMap::new(),
        hooks: HookConfiguration {
            turn_start: vec![],
            turn_stop: vec![],
        },
    }
}

fn session(config_dir: &std::path::Path) -> String {
    SessionStore::with_config_dir(config_dir)
        .unwrap()
        .create(None, None, None)
        .unwrap()
        .id
}

fn complete(message: &str) -> Vec<Result<StreamEvent, ProviderError>> {
    vec![
        Ok(StreamEvent::TextDelta(message.into())),
        Ok(StreamEvent::Complete(Response {
            message: Some(message.into()),
            reasoning: None,
            tool_calls: None,
            usage: None,
            model: None,
        })),
    ]
}

#[test]
fn run_loop_success_completes_and_cleans_up_lifecycle() {
    let env = support::TestEnv::new();
    let id = session(&env.dir());
    let mut ctx = build_context(&id, &effective(env.dir()), &env.dir()).unwrap();
    let provider = SequenceProvider::new(vec![Ok(complete("done"))]);
    assert_eq!(run_loop(&mut ctx, &provider), Ok(()));
    let state = SessionStore::with_config_dir(&env.dir())
        .unwrap()
        .state(&id)
        .unwrap();
    assert_eq!(state.status, Status::Idle);
    assert!(state.pid.is_none());
    assert_eq!(state.last_message.as_deref(), Some("done"));
    assert_eq!(provider.calls(), 1);
}

#[test]
fn run_loop_provider_failure_marks_session_failed_and_cleans_up() {
    let env = support::TestEnv::new();
    let id = session(&env.dir());
    let mut ctx = build_context(&id, &effective(env.dir()), &env.dir()).unwrap();
    let provider = SequenceProvider::new(vec![Err(ProviderError::Network("offline".into()))]);
    let error = run_loop(&mut ctx, &provider).unwrap_err();
    assert!(error.contains("provider error"));
    let state = SessionStore::with_config_dir(&env.dir())
        .unwrap()
        .state(&id)
        .unwrap();
    assert_eq!(state.status, Status::Failed);
    assert!(state.pid.is_none());
    assert_eq!(provider.calls(), 1);
}

#[test]
fn run_loop_tool_call_continues_until_provider_completes() {
    let env = support::TestEnv::new();
    let id = session(&env.dir());
    let mut config = effective(env.dir());
    config.permissions.tools = vec!["bash".into()];
    let mut ctx = build_context(&id, &config, &env.dir()).unwrap();
    let call = ToolCall {
        id: "call-1".into(),
        name: "bash".into(),
        input: serde_json::json!({"cmd": "printf tool-ok"}),
    };
    let first = vec![Ok(StreamEvent::Complete(Response {
        message: Some("using tool".into()),
        reasoning: None,
        tool_calls: Some(vec![call]),
        usage: None,
        model: None,
    }))];
    let provider = SequenceProvider::new(vec![Ok(first), Ok(complete("finished"))]);
    run_loop(&mut ctx, &provider).unwrap();
    let transcript = std::fs::read_to_string(
        env.dir()
            .join("sessions")
            .join(&id)
            .join("conversation.jsonl"),
    )
    .unwrap();
    assert!(transcript.contains("tool_call"));
    assert!(transcript.contains("tool_result"));
    assert!(transcript.contains("tool-ok"));
    assert!(transcript.contains("finished"));
    assert_eq!(provider.calls(), 2);
}

#[test]
fn run_loop_budget_stop_does_not_call_provider_and_cleans_up() {
    let env = support::TestEnv::new();
    let id = session(&env.dir());
    let mut ctx = build_context_with_budget(
        &id,
        &effective(env.dir()),
        &env.dir(),
        &TokenBudget {
            warn_threshold: 0,
            hard_limit: 0,
        },
    )
    .unwrap();
    let provider = SequenceProvider::new(vec![]);
    let error = run_loop(&mut ctx, &provider).unwrap_err();
    assert!(error.contains("before sending"));
    assert_eq!(provider.calls(), 0);
    let state = SessionStore::with_config_dir(&env.dir())
        .unwrap()
        .state(&id)
        .unwrap();
    assert_eq!(state.status, Status::Failed);
    assert!(state.pid.is_none());
    assert!(state.last_message.unwrap().contains("SESSION TERMINATED"));
}

#[test]
fn run_loop_stream_failure_is_cleaned_up_by_guard() {
    let env = support::TestEnv::new();
    let id = session(&env.dir());
    let mut ctx = build_context(&id, &effective(env.dir()), &env.dir()).unwrap();
    let provider = SequenceProvider::new(vec![Ok(vec![Err(ProviderError::Network(
        "dropped".into(),
    ))])]);
    assert!(run_loop(&mut ctx, &provider).is_err());
    let state = SessionStore::with_config_dir(&env.dir())
        .unwrap()
        .state(&id)
        .unwrap();
    assert_eq!(state.status, Status::Failed);
    assert!(state.pid.is_none());
}

#[test]
fn cancellation_contract_is_covered_by_lifecycle_tests() {
    let env = support::TestEnv::new();
    let id = session(&env.dir());
    orchid::r#loop::lifecycle::on_run_start(&id, &env.dir()).unwrap();
    orchid::r#loop::lifecycle::on_run_end_with_status(&id, Status::Cancelled, &env.dir()).unwrap();
    let state = SessionStore::with_config_dir(&env.dir())
        .unwrap()
        .state(&id)
        .unwrap();
    assert_eq!(state.status, Status::Cancelled);
    assert!(state.pid.is_none());
}
