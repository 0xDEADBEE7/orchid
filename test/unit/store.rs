use orchid::{model::Session, Store};

#[test]
fn worker_save_does_not_restore_an_externally_edited_agent() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path()).unwrap();
    let session = Session::new(None, None, None);
    let id = session.metadata.id.clone();
    store.create(&session).unwrap();

    store
        .update(&id, |session| {
            session.metadata.agent.max_tokens = Some(120_000)
        })
        .unwrap();

    let mut stale = store.load(&id).unwrap();
    stale.metadata.agent.max_tokens = Some(1);
    stale.metadata.token_estimate = 60_000;
    store.save(&stale).unwrap();

    assert_eq!(
        store.load(&id).unwrap().metadata.agent.max_tokens,
        Some(120_000)
    );
}

#[test]
fn round_trips_and_archives_sessions() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path()).unwrap();
    let mut session = Session::new(Some("test".into()), None, None);
    session.append(Session::message("user", "hello".into()));
    store.create(&session).unwrap();
    let session_dir = dir.path().join("sessions").join(&session.metadata.id);
    assert!(session_dir.join("metadata.json").exists());
    assert!(!session_dir.join("state.json").exists());
    assert_eq!(store.load(&session.metadata.id).unwrap(), session);
    store.archive(&session.metadata.id).unwrap();
    assert!(store.load(&session.metadata.id).is_err());
}

#[test]
fn reconciles_dead_running_processes() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path()).unwrap();
    let mut session = Session::new(None, None, None);
    session.metadata.status = orchid::model::Status::Running;
    session.metadata.pid = Some(999_999);
    store.create(&session).unwrap();
    let recovered = store.reconcile(&session.metadata.id).unwrap();
    assert_eq!(recovered.metadata.status, orchid::model::Status::Failed);
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
    session.metadata.label = Some("changed metadata only".into());
    store.save(&session).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), first);
    session.append(Session::message("assistant", "two".into()));
    store.save(&session).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 2);
}

#[test]
fn events_capture_the_active_connection() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path()).unwrap();
    let mut session = Session::new(None, None, None);
    session.metadata.agent.connection = "premium".into();
    session.append(Session::message("user", "hello".into()));
    store.create(&session).unwrap();

    let line = std::fs::read_to_string(
        dir.path()
            .join("sessions")
            .join(&session.metadata.id)
            .join("events.jsonl"),
    )
    .unwrap();
    let event: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(event["connection"], "premium");
}

#[test]
fn legacy_events_without_a_connection_still_deserialize() {
    let event: orchid::model::Event = serde_json::from_value(serde_json::json!({
        "type": "message",
        "event_id": "legacy",
        "timestamp": "2026-01-01T00:00:00Z",
        "role": "user",
        "content": "hello",
        "token_usage": {
            "marginal_input": 0,
            "cumulative_input": 0,
            "cumulative_output": 0,
            "cumulative_cached_input": 0,
            "requests": 0,
            "method": ""
        }
    }))
    .unwrap();
    assert!(matches!(
        event,
        orchid::model::Event::Message {
            connection: None,
            ..
        }
    ));
}

#[test]
fn debug_logs_are_filtered_by_configured_level() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path()).unwrap();
    let session = Session::new(None, None, None);
    let id = session.metadata.id.clone();
    store.create(&session).unwrap();
    let record = orchid::model::LogRecord {
        event_id: uuid::Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now(),
        level: "debug".into(),
        message: "transport".into(),
        fields: serde_json::json!({}),
    };
    store.log_both(&id, &record, "info").unwrap();
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

#[test]
fn messages_queue_while_a_worker_owns_the_session_and_drain_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path()).unwrap();
    let session = Session::new(None, None, None);
    let id = session.metadata.id.clone();
    store.create(&session).unwrap();
    // A marker file left by the old lock implementation must not wedge writes.
    std::fs::write(dir.path().join("sessions").join(&id).join(".lock"), "").unwrap();
    // The first delivery claims the session for this (live) process.
    let mut owned = store
        .deliver(&id, Session::message("user", "one".into()), true)
        .unwrap()
        .unwrap();
    assert_eq!(owned.metadata.pid, Some(std::process::id()));
    for text in ["two", "three"] {
        assert!(store
            .deliver(&id, Session::message("user", text.into()), false)
            .unwrap()
            .is_none());
    }
    assert_eq!(store.load(&id).unwrap().events.len(), 1);
    assert!(store.drain(&mut owned).unwrap());
    let texts: Vec<_> = owned
        .events
        .iter()
        .filter_map(|e| match e {
            orchid::model::Event::Message { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, ["one", "two", "three"]);
    assert!(!store.finish(&mut owned).unwrap());
    let done = store.load(&id).unwrap();
    assert_eq!(
        (done.metadata.status, done.metadata.pid, done.events.len()),
        (orchid::Status::Idle, None, 3)
    );
}

#[test]
fn stale_saves_keep_label_and_working_dir_set_meanwhile() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path()).unwrap();
    let session = Session::new(None, None, None);
    let id = session.metadata.id.clone();
    store.create(&session).unwrap();
    let stale = store.load(&id).unwrap();
    store
        .update(&id, |s| {
            s.metadata.label = Some("renamed".into());
            s.metadata.working_dir = Some("/tmp".into());
        })
        .unwrap();
    store.save(&stale).unwrap();
    let saved = store.load(&id).unwrap().metadata;
    assert_eq!(
        (saved.label.as_deref(), saved.working_dir.as_deref()),
        (Some("renamed"), Some("/tmp"))
    );
}

#[test]
fn reconcile_answers_open_tool_calls_of_a_dead_worker() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path()).unwrap();
    let mut session = Session::new(None, None, None);
    session.append(orchid::model::Event::ToolCall {
        event_id: "e".into(),
        timestamp: chrono::Utc::now(),
        calls: vec![orchid::model::ToolCall {
            call_id: "c1".into(),
            name: "bash".into(),
            input: serde_json::json!({}),
        }],
        connection: None,
        token_usage: Default::default(),
    });
    session.metadata.status = orchid::Status::Running;
    session.metadata.pid = Some(u32::MAX - 1);
    let id = session.metadata.id.clone();
    store.create(&session).unwrap();
    let reconciled = store.reconcile(&id).unwrap();
    assert_eq!(reconciled.metadata.status, orchid::Status::Failed);
    assert!(reconciled.pending_tool_call_ids().is_empty());
}
