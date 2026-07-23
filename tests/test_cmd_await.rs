use orchid::cmd::await_sessions;
use orchid::parse_args;
use orchid::session::SessionUpdate;
use orchid::types::Status;
use orchid::Command;
use orchid::SessionStore;

mod support;
use support::TestEnv;

#[test]
#[serial_test::serial]
fn await_returns_already_idle_session() {
    let env = TestEnv::new();
    let store = SessionStore::with_config_dir(&env.dir()).unwrap();
    let metadata = store.create(None, None, None).unwrap();

    let (result, code) = await_sessions(vec![metadata.id.clone()], 1.0, 0.01, &env.dir()).unwrap();

    assert_eq!(code, 0);
    assert_eq!(result["completed"][0]["id"], metadata.id);
    assert_eq!(result["completed"][0]["status"], "idle");
}

#[test]
#[serial_test::serial]
fn await_returns_all_terminal_sessions_and_deduplicates_ids() {
    let env = TestEnv::new();
    let store = SessionStore::with_config_dir(&env.dir()).unwrap();
    let first = store.create(None, None, None).unwrap();
    let second = store.create(None, None, None).unwrap();
    store.update(&first.id, SessionUpdate { status: Some(Status::Failed), ..Default::default() }).unwrap();
    store.update(&second.id, SessionUpdate { status: Some(Status::Cancelled), ..Default::default() }).unwrap();

    let (result, code) = await_sessions(
        vec![first.id.clone(), first.id, second.id.clone()],
        1.0,
        0.01,
        &env.dir(),
    ).unwrap();

    assert_eq!(code, 0);
    assert_eq!(result["completed"].as_array().unwrap().len(), 2);
    assert_eq!(result["completed"][0]["status"], "failed");
    assert_eq!(result["completed"][1]["status"], "cancelled");
}

#[test]
#[serial_test::serial]
fn await_times_out_when_session_is_running() {
    let env = TestEnv::new();
    let store = SessionStore::with_config_dir(&env.dir()).unwrap();
    let metadata = store.create(None, None, None).unwrap();
    store.update(&metadata.id, SessionUpdate { status: Some(Status::Running), ..Default::default() }).unwrap();

    let (result, code) = await_sessions(vec![metadata.id], 0.02, 0.005, &env.dir()).unwrap();

    assert_eq!(code, 2);
    assert_eq!(result, serde_json::json!({"completed": [], "timed_out": true}));
}

#[test]
fn await_rejects_missing_and_invalid_options() {
    assert_eq!(
        parse_args(&["await".to_string()]),
        Err("await requires at least one session ID".to_string())
    );
    assert_eq!(
        parse_args(&["await".to_string(), "a".repeat(32), "--timeout=-1".to_string()]),
        Err("invalid timeout value: -1".to_string())
    );
    assert_eq!(
        parse_args(&["await".to_string(), "a".repeat(32), "--interval=nan".to_string()]),
        Err("invalid interval value: nan".to_string())
    );
}

#[test]
fn await_parses_options() {
    let (command, _) = parse_args(&[
        "await".to_string(),
        "a".repeat(32),
        "--timeout".to_string(),
        "10.5".to_string(),
        "--interval=0.25".to_string(),
    ]).unwrap();

    assert_eq!(command, Command::Await {
        ids: vec!["a".repeat(32)],
        timeout: 10.5,
        interval: 0.25,
    });
}
