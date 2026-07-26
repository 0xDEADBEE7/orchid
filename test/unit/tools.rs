use orchid::tools::{edit, read};
use std::io;
use orchid::config::{init, Settings};

#[test]
fn policy_denies_tools_by_default() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path()).unwrap();
    assert_eq!(
        read(&Settings::load(dir.path()).unwrap(), &["x".into()])
            .unwrap_err()
            .kind(),
        io::ErrorKind::PermissionDenied
    );
}

#[test]
fn policy_paths_narrow_file_scope() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path()).unwrap();
    std::fs::create_dir(dir.path().join("allowed")).unwrap();
    std::fs::write(dir.path().join("allowed/file"), "ok").unwrap();
    std::fs::write(
        dir.path().join("policies/default.json"),
        r#"{"tools":["fs_read"],"paths":["allowed"]}"#,
    )
    .unwrap();
    let settings = Settings::load(dir.path()).unwrap();
    assert!(read(&settings, &["allowed/file".into()]).is_ok());
    assert_eq!(
        read(&settings, &["config.json".into()]).unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
}

#[test]
fn fs_read_supports_batch_calls() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path()).unwrap();
    std::fs::write(dir.path().join("a"), "A").unwrap();
    std::fs::write(dir.path().join("b"), "B").unwrap();
    let mut policy = Settings::load(dir.path()).unwrap().policy;
    policy.tools = vec!["fs_read".into()];
    let settings = Settings {
        root: dir.path().into(),
        policy_name: "default".into(),
        policy,
        prompt_name: "default".into(),
        log_level: "info".into(),
    };
    assert_eq!(
        read(&settings, &["a".into(), "b".into()]).unwrap(),
        serde_json::json!({"a":"A","b":"B"})
    );
}

#[test]
fn fs_edit_applies_structured_edits() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path()).unwrap();
    std::fs::write(dir.path().join("file"), "old old").unwrap();
    let mut policy = Settings::load(dir.path()).unwrap().policy;
    policy.tools = vec!["fs_edit".into()];
    let settings = Settings {
        root: dir.path().into(),
        policy_name: "default".into(),
        policy,
        prompt_name: "default".into(),
        log_level: "info".into(),
    };
    edit(
        &settings,
        "file",
        &serde_json::json!([{"old_string":"old","new_string":"new","replace_all":true}]),
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("file")).unwrap(),
        "new new"
    );
}

#[test]
fn fs_edit_reports_ambiguous_batch_edit_index() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path()).unwrap();
    std::fs::write(dir.path().join("file"), "same same").unwrap();
    let mut policy = Settings::load(dir.path()).unwrap().policy;
    policy.tools = vec!["fs_edit".into()];
    let settings = Settings {
        root: dir.path().into(),
        policy_name: "default".into(),
        policy,
        prompt_name: "default".into(),
        log_level: "info".into(),
    };
    let error = edit(
        &settings,
        "file",
        &serde_json::json!([
            {"old_string":"missing","new_string":"x"},
            {"old_string":"same","new_string":"new"}
        ]),
    )
    .unwrap_err();
    assert!(error.to_string().contains("edit[0]"));
}
