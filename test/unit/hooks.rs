use orchid::{
    config::{init, HookDefinition, Settings},
    hooks::{append, dispatch, resolve_executable},
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
    settings.hooks.insert(
        "on-event".into(),
        vec![HookDefinition {
            script: "hook.sh".into(),
            timeout: Some(2),
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
        orchid::config::read_json(&dir.path().join("received.json")).unwrap();
    assert_eq!(received["version"], 1);
    assert_eq!(received["event"]["name"], "on-event");
    assert_eq!(received["session"]["events"].as_array().unwrap().len(), 1);
    assert_eq!(store.load(&session.metadata.id).unwrap().events.len(), 1);
}

#[test]
fn global_hooks_run_before_agent_hooks_in_declared_order() {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = settings(dir.path());
    script(dir.path(), "one.sh", "echo one >> order");
    script(dir.path(), "two.sh", "echo two >> order");
    settings.hooks.insert(
        "on-event".into(),
        vec![HookDefinition {
            script: "one.sh".into(),
            timeout: None,
        }],
    );
    // Agent hooks run after the global ones.
    settings.agent.hooks.insert(
        "on-event".into(),
        vec![HookDefinition {
            script: "two.sh".into(),
            timeout: None,
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
    settings.hooks.insert(
        "on-event".into(),
        vec![HookDefinition {
            script: "slow.sh".into(),
            timeout: Some(0),
        }],
    );
    let mut session = Session::new(None, None, None);
    let event = Session::message("user", "hello".into());
    session.append(event.clone());
    let error = dispatch(&settings, "on-event", &event, &session);
    assert!(error.is_err());
}

#[test]
fn executable_resolution_distinguishes_path_like_and_bare_names() {
    let dir = tempfile::tempdir().unwrap();
    let settings = settings(dir.path());
    assert_eq!(
        resolve_executable(&settings, "hooks/test.sh"),
        dir.path().join("hooks/test.sh")
    );
    assert_eq!(
        resolve_executable(&settings, "/bin/ls"),
        std::path::PathBuf::from("/bin/ls")
    );
    assert_eq!(
        resolve_executable(&settings, "ls"),
        std::path::PathBuf::from("ls")
    );
    std::fs::write(dir.path().join("ls"), "not executable").unwrap();
    assert_eq!(
        resolve_executable(&settings, "ls"),
        std::path::PathBuf::from("ls")
    );
}

#[test]
fn hooks_run_from_session_working_directory() {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = settings(dir.path());
    std::fs::create_dir(dir.path().join("work")).unwrap();
    script(dir.path(), "pwd.sh", "pwd > cwd");
    settings.hooks.insert(
        "on-event".into(),
        vec![HookDefinition {
            script: "pwd.sh".into(),
            timeout: None,
        }],
    );
    let work = dir.path().join("work").to_string_lossy().into_owned();
    let mut session = Session::new(None, Some(work), None);
    let event = Session::message("user", "hello".into());
    session.append(event.clone());
    dispatch(&settings, "on-event", &event, &session).unwrap();
    let actual = std::fs::read_to_string(dir.path().join("work/cwd")).unwrap();
    assert_eq!(
        std::path::Path::new(actual.trim()).canonicalize().unwrap(),
        dir.path().join("work").canonicalize().unwrap()
    );
}
