use super::*;
use crate::model::Session;

#[test]
fn round_trips_and_archives_sessions() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path()).unwrap();
    let mut session = Session::new(Some("test".into()), None, None);
    session.append(Session::message("user", "hello".into()));
    store.create(&session).unwrap();
    assert_eq!(store.load(&session.metadata.id).unwrap(), session);
    store.archive(&session.metadata.id).unwrap();
    assert!(store.load(&session.metadata.id).is_err());
}

#[test]
fn reconciles_dead_running_processes() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path()).unwrap();
    let mut session = Session::new(None, None, None);
    session.state.status = crate::model::Status::Running;
    session.state.pid = Some(999_999);
    store.create(&session).unwrap();
    let recovered = store.reconcile(&session.metadata.id).unwrap();
    assert_eq!(recovered.state.status, crate::model::Status::Failed);
}

#[test]
fn event_stream_only_grows_on_save() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path()).unwrap();
    let mut session = Session::new(None, None, None);
    store.create(&session).unwrap();
    let path = dir
        .path()
        .join("sessions")
        .join(&session.metadata.id)
        .join("events.jsonl");
    session.append(Session::message("user", "one".into()));
    store.save(&session).unwrap();
    let first = std::fs::read_to_string(&path).unwrap();
    session.state.last_message = Some("changed state only".into());
    store.save(&session).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), first);
    session.append(Session::message("assistant", "two".into()));
    store.save(&session).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 2);
}

#[test]
fn debug_logs_are_filtered_by_configured_level() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path()).unwrap();
    let session = Session::new(None, None, None);
    let id = session.metadata.id.clone();
    store.create(&session).unwrap();
    let record = crate::model::LogRecord {
        event_id: uuid::Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now(),
        level: "debug".into(),
        message: "transport".into(),
        fields: serde_json::json!({}),
    };
    store.log_filtered(&id, &record, "info").unwrap();
    assert!(
        std::fs::read_to_string(dir.path().join("sessions").join(id).join("logs.jsonl"))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn concurrent_appends_are_serialized_across_store_clones() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path()).unwrap();
    let session = Session::new(None, None, None);
    let id = session.metadata.id.clone();
    store.create(&session).unwrap();
    let mut workers = Vec::new();
    for index in 0..8 {
        let store = store.clone();
        let id = id.clone();
        workers.push(std::thread::spawn(move || {
            store
                .append_event(&id, Session::message("user", index.to_string()))
                .unwrap();
        }));
    }
    for worker in workers {
        worker.join().unwrap();
    }
    assert_eq!(store.load(&id).unwrap().events.len(), 8);
}
