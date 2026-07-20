use orchid::convo::Store;
use orchid::r#loop::lifecycle::{on_run_end, on_run_start};
use orchid::Status;
mod support;
use support::TestEnv;

#[test]
#[serial_test::serial]
fn test_on_run_start() {
    let env = TestEnv::new();
    let orchid_dir = env.dir();
    let convos_dir = orchid_dir.join("conversations");
    std::fs::create_dir_all(&convos_dir).unwrap();
    let store = Store::with_base(convos_dir);
    let meta = store.create(None, None, None, None, None).unwrap();
    on_run_start(&meta.id).ok();
    let updated = store.get(&meta.id).unwrap();
    assert_eq!(updated.status, Status::Running);
    assert!(updated.pid.is_some());
}

#[test]
#[serial_test::serial]
fn test_on_run_end() {
    let env = TestEnv::new();
    let orchid_dir = env.dir();
    let convos_dir = orchid_dir.join("conversations");
    std::fs::create_dir_all(&convos_dir).unwrap();
    let store = Store::with_base(convos_dir);
    let meta = store.create(None, None, None, None, None).unwrap();
    on_run_start(&meta.id).ok();
    on_run_end(&meta.id).ok();
    let updated = store.get(&meta.id).unwrap();
    assert_eq!(updated.status, Status::Idle);
    assert!(updated.pid.is_none());
    assert!(updated.last_run_at.is_some());
    assert!(
        updated.run_started_at.is_none(),
        "run_started_at must be cleared on run end"
    );
}
