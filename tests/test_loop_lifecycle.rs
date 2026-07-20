use orchid::r#loop::lifecycle::{on_run_end, on_run_start};
use orchid::SessionStore as Store;
use orchid::Status;
mod support;
use support::TestEnv;
#[test]
#[serial_test::serial]
fn failed_metadata_write_does_not_change_existing_metadata() {
    let env = TestEnv::new();
    let config_dir = env.dir();
    let store = Store::with_config_dir(&config_dir).unwrap();
    let meta = store.create(Some("original".into()), None, None).unwrap();
    let original = std::fs::read_to_string(store.metadata_path(&meta.id)).unwrap();

    let temp = store.session_path(&meta.id).join(".metadata.json.tmp");
    std::fs::create_dir(&temp).unwrap();
    let result = store.update(
        &meta.id,
        orchid::SessionUpdate {
            label: Some(Some("changed".into())),
            ..Default::default()
        },
    );

    assert!(result.is_err());
    assert_eq!(
        std::fs::read_to_string(store.metadata_path(&meta.id)).unwrap(),
        original
    );
    std::fs::remove_dir(temp).unwrap();
}

#[test]
#[serial_test::serial]
fn failed_state_write_does_not_change_existing_state() {
    let env = TestEnv::new();
    let config_dir = env.dir();
    let store = Store::with_config_dir(&config_dir).unwrap();
    let meta = store.create(None, None, None).unwrap();
    let original = std::fs::read_to_string(store.state_path(&meta.id)).unwrap();

    let temp = store.session_path(&meta.id).join(".state.json.tmp");
    std::fs::create_dir(&temp).unwrap();
    let result = store.update(
        &meta.id,
        orchid::SessionUpdate {
            status: Some(Status::Running),
            ..Default::default()
        },
    );

    assert!(result.is_err());
    assert_eq!(
        std::fs::read_to_string(store.state_path(&meta.id)).unwrap(),
        original
    );
    std::fs::remove_dir(temp).unwrap();
}

#[test]
#[serial_test::serial]
fn test_on_run_start() {
    let env = TestEnv::new();
    let orchid_dir = env.dir();
    let sessions_dir = orchid_dir.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    let store = Store::with_base(sessions_dir);
    let meta = store.create(None, None, None).unwrap();
    on_run_start(&meta.id, &orchid_dir).ok();
    let _updated = store.get(&meta.id).unwrap();
    let state = store.state(&meta.id).unwrap();
    assert_eq!(state.status, Status::Running);
    assert!(state.pid.is_some());
}

#[test]
#[serial_test::serial]
fn test_on_run_end() {
    let env = TestEnv::new();
    let orchid_dir = env.dir();
    let sessions_dir = orchid_dir.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    let store = Store::with_base(sessions_dir);
    let meta = store.create(None, None, None).unwrap();
    on_run_start(&meta.id, &orchid_dir).ok();
    on_run_end(&meta.id, &orchid_dir).ok();
    let _updated = store.get(&meta.id).unwrap();
    let state = store.state(&meta.id).unwrap();
    assert_eq!(state.status, Status::Idle);
    assert!(state.pid.is_none());
    assert!(state.last_run_at.is_some());
    assert!(
        state.run_started_at.is_none(),
        "run_started_at must be cleared on run end"
    );
}

#[test]
#[serial_test::serial]
fn test_missing_state_is_reconciled() {
    let env = TestEnv::new();
    let config_dir = env.dir();
    let store = Store::with_config_dir(&config_dir).unwrap();
    let meta = store.create(None, None, None).unwrap();
    std::fs::remove_file(store.state_path(&meta.id)).unwrap();

    assert!(orchid::r#loop::lifecycle::detect_crashed(&meta.id, &config_dir).unwrap());
    orchid::r#loop::lifecycle::reconcile_crashed(&meta.id, &config_dir).unwrap();
    assert_eq!(store.state(&meta.id).unwrap().status, Status::Idle);
}

#[test]
#[serial_test::serial]
fn test_malformed_state_is_reconciled() {
    let env = TestEnv::new();
    let config_dir = env.dir();
    let store = Store::with_config_dir(&config_dir).unwrap();
    let meta = store.create(None, None, None).unwrap();
    std::fs::write(store.state_path(&meta.id), "not-json").unwrap();

    assert!(orchid::r#loop::lifecycle::detect_crashed(&meta.id, &config_dir).unwrap());
    orchid::r#loop::lifecycle::reconcile_crashed(&meta.id, &config_dir).unwrap();
    assert_eq!(store.state(&meta.id).unwrap().status, Status::Idle);
}

#[test]
#[serial_test::serial]
fn test_invalid_pid_is_crashed() {
    let env = TestEnv::new();
    let config_dir = env.dir();
    let store = Store::with_config_dir(&config_dir).unwrap();
    let meta = store.create(None, None, None).unwrap();
    store
        .update(
            &meta.id,
            orchid::SessionUpdate {
                status: Some(Status::Running),
                pid: Some(Some(0)),
                ..Default::default()
            },
        )
        .unwrap();

    assert!(orchid::r#loop::lifecycle::detect_crashed(&meta.id, &config_dir).unwrap());
}
