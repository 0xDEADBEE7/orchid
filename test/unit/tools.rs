use orchid::config::{init, Settings};
use serde_json::Value;
use std::io;

fn read(settings: &Settings, paths: &[String]) -> io::Result<Value> {
    orchid::tools::read_from(settings, paths, &settings.root)
}

fn edit(settings: &Settings, path: &str, edits: &Value) -> io::Result<Value> {
    orchid::tools::edit_from(settings, path, edits, &settings.root)
}

fn with_tools(dir: &std::path::Path, tools: &[&str]) -> Settings {
    init(dir).unwrap();
    let mut settings = Settings::load(dir).unwrap();
    settings.agent.tools = tools.iter().map(|tool| tool.to_string()).collect();
    settings
}

#[test]
fn tools_are_denied_unless_the_agent_enables_them() {
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
fn file_tools_stay_inside_the_working_directory() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("work");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("file"), "ok").unwrap();
    std::fs::write(dir.path().join("outside"), "no").unwrap();
    let settings = with_tools(&root, &["fs_read"]);
    assert!(read(&settings, &["file".into()]).is_ok());
    assert_eq!(
        read(&settings, &["../outside".into()]).unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
}

#[test]
fn fs_read_supports_batch_calls() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a"), "A").unwrap();
    std::fs::write(dir.path().join("b"), "B").unwrap();
    let settings = with_tools(dir.path(), &["fs_read"]);
    assert_eq!(
        read(&settings, &["a".into(), "b".into()]).unwrap(),
        serde_json::json!({"a":"A","b":"B"})
    );
}

#[test]
fn fs_edit_applies_structured_edits() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("file"), "old old").unwrap();
    let settings = with_tools(dir.path(), &["fs_edit"]);
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
    std::fs::write(dir.path().join("file"), "same same").unwrap();
    let settings = with_tools(dir.path(), &["fs_edit"]);
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
