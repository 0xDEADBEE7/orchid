use orchid::r#loop::lifecycle::{on_run_end, on_run_start};
use orchid::SessionStore as Store;
use orchid::Status;
mod support;
use support::TestEnv;

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
