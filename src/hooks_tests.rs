use super::*;
use crate::{
    config::{init, HookDefinition, HookMode, Settings},
    model::Session,
    store::Store,
};
use std::{fs, os::unix::fs::PermissionsExt};

fn settings(dir: &std::path::Path) -> Settings {
    init(dir).unwrap();
    Settings::load(dir).unwrap()
}

fn script(dir: &std::path::Path, name: &str, body: &str) {
    let path = dir.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn sync_hook_receives_versioned_session_envelope_after_persist() {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = settings(dir.path());
    script(dir.path(), "hook.sh", "cat > received.json");
    settings.policy.hooks.events.insert(
        "on-event".into(),
        vec![HookDefinition {
            script: "hook.sh".into(),
            mode: HookMode::Sync,
            timeout_seconds: Some(2),
        }],
    );
    let store = Store::new(dir.path()).unwrap();
    let mut session = Session::new(None, None, None);
    store.create(&session).unwrap();
    append(
        &store,
        &settings,
        &mut session,
        Session::message("user", "hello".into()),
    )
    .unwrap();
    let received: serde_json::Value =
        crate::config::read_json(&dir.path().join("received.json")).unwrap();
    assert_eq!(received["version"], 1);
    assert_eq!(received["event"]["name"], "on-event");
    assert_eq!(received["session"]["events"].as_array().unwrap().len(), 1);
    assert_eq!(store.load(&session.metadata.id).unwrap().events.len(), 1);
}

#[test]
fn matching_hooks_launch_in_declared_order() {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = settings(dir.path());
    script(dir.path(), "one.sh", "echo one >> order");
    script(dir.path(), "two.sh", "echo two >> order");
    settings.policy.hooks.events.insert(
        "on-event".into(),
        vec![
            HookDefinition {
                script: "one.sh".into(),
                mode: HookMode::Sync,
                timeout_seconds: None,
            },
            HookDefinition {
                script: "two.sh".into(),
                mode: HookMode::Sync,
                timeout_seconds: None,
            },
        ],
    );
    let store = Store::new(dir.path()).unwrap();
    let mut session = Session::new(None, None, None);
    store.create(&session).unwrap();
    append(
        &store,
        &settings,
        &mut session,
        Session::message("user", "hello".into()),
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("order")).unwrap(),
        "one\ntwo\n"
    );
}

#[test]
fn sync_hook_timeout_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = settings(dir.path());
    script(dir.path(), "slow.sh", "sleep 1");
    settings.policy.hooks.events.insert(
        "on-event".into(),
        vec![HookDefinition {
            script: "slow.sh".into(),
            mode: HookMode::Sync,
            timeout_seconds: Some(0),
        }],
    );
    let mut session = Session::new(None, None, None);
    let event = Session::message("user", "hello".into());
    session.append(event.clone());
    let error = dispatch(&settings, "on-event", &event, &session);
    assert!(error.is_err());
}

#[test]
fn async_hook_is_launched_without_blocking_dispatch() {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = settings(dir.path());
    script(
        dir.path(),
        "async.sh",
        "sleep 0.1; echo done > async-result",
    );
    settings.policy.hooks.events.insert(
        "on-event".into(),
        vec![HookDefinition {
            script: "async.sh".into(),
            mode: HookMode::Async,
            timeout_seconds: Some(2),
        }],
    );
    let mut session = Session::new(None, None, None);
    let event = Session::message("user", "hello".into());
    session.append(event.clone());
    let started = std::time::Instant::now();
    dispatch(&settings, "on-event", &event, &session).unwrap();
    assert!(started.elapsed() < std::time::Duration::from_millis(90));
    for _ in 0..30 {
        if dir.path().join("async-result").exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("async hook did not complete");
}
