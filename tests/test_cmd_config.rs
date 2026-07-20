use orchid::cmd::{config_list, config_show};
use std::fs;

#[test]
fn test_config_list_resources() {
    let dir = tempfile::tempdir().unwrap();
    for kind in ["connections", "policies", "prompts"] {
        fs::create_dir_all(dir.path().join(kind)).unwrap();
    }
    fs::write(dir.path().join("connections/local.json"), "{}").unwrap();
    let result = config_list(dir.path()).unwrap();
    assert!(result.get("connections").is_some());
}

#[test]
fn test_config_show_root() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("config.json"), r#"{"policy":"default"}"#).unwrap();
    let result = config_show(dir.path(), "root").unwrap();
    assert_eq!(result["policy"], "default");
}
