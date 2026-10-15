use std::{fs, process::Command};

fn run(binary: &str, root: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(binary)
        .arg("--config")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn a_hook_exiting_2_keeps_a_user_message_out_of_the_transcript() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    orchid::config::init(root).unwrap();
    let binary = env!("CARGO_BIN_EXE_orchid");
    fs::write(
        root.join("deny.sh"),
        "#!/bin/sh\necho 'not today' >&2\nexit 2\n",
    )
    .unwrap();
    set_executable(&root.join("deny.sh"));
    fs::write(
        root.join("config.json"),
        r#"{"hooks":{"on-event":[{"script":"deny.sh"}]}}"#,
    )
    .unwrap();
    let created: serde_json::Value =
        serde_json::from_slice(&run(binary, root, &["create"]).stdout).unwrap();
    let id = created["id"].as_str().unwrap();
    let sent = run(binary, root, &["send", "--no-run", "--id", id, "hello"]);
    assert!(!sent.status.success());
    assert!(String::from_utf8_lossy(&sent.stderr).contains("blocked by hook: not today"));
    let session: serde_json::Value =
        serde_json::from_slice(&run(binary, root, &["get", id]).stdout).unwrap();
    assert!(session["events"].as_array().unwrap().is_empty());
}

#[test]
fn session_metadata_keeps_an_independent_agent_policy_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    orchid::config::init(root).unwrap();
    let binary = env!("CARGO_BIN_EXE_orchid");

    fs::write(
        root.join("agents/default.json"),
        r#"{"prompt":"default","max_tokens":120000,"tools":["bash"]}"#,
    )
    .unwrap();
    let created = run(binary, root, &["create"]);
    assert!(created.status.success());
    let created: serde_json::Value = serde_json::from_slice(&created.stdout).unwrap();
    let id = created["id"].as_str().unwrap();

    fs::write(
        root.join("agents/default.json"),
        r#"{"prompt":"default","max_tokens":4096,"tools":[]}"#,
    )
    .unwrap();
    let loaded = run(binary, root, &["get", id]);
    assert!(loaded.status.success());
    let loaded: serde_json::Value = serde_json::from_slice(&loaded.stdout).unwrap();
    assert_eq!(loaded["metadata"]["agent"]["max_tokens"], 120000);
    assert_eq!(loaded["metadata"]["agent"]["tools"][0], "bash");
    assert_eq!(loaded["metadata"]["agent_name"], "default");
    fs::write(root.join("prompts/default.txt"), "A changed source prompt.").unwrap();
    assert_eq!(
        fs::read_to_string(root.join("sessions").join(id).join("prompt.md")).unwrap(),
        "You are a helpful assistant."
    );
}

#[test]
fn list_returns_only_session_ids_and_labels() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    orchid::config::init(root).unwrap();
    let binary = env!("CARGO_BIN_EXE_orchid");

    let first = run(binary, root, &["create", "--label", "first"]);
    assert!(first.status.success());
    let first: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    let first_id = first["id"].as_str().unwrap();

    let second = run(binary, root, &["create"]);
    assert!(second.status.success());
    let second: serde_json::Value = serde_json::from_slice(&second.stdout).unwrap();
    let second_id = second["id"].as_str().unwrap();

    let listed = run(binary, root, &["list"]);
    assert!(listed.status.success());
    let listed: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    let sessions = listed["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 2);
    assert_eq!(
        sessions[0],
        serde_json::json!({"id": first_id, "label": "first"})
    );
    assert_eq!(
        sessions[1],
        serde_json::json!({"id": second_id, "label": null})
    );
}

#[cfg(unix)]
#[test]
fn stop_kills_worker_marks_session_idle_and_allows_follow_up() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    orchid::config::init(root).unwrap();
    let binary = env!("CARGO_BIN_EXE_orchid");

    let created = run(binary, root, &["create"]);
    assert!(created.status.success());
    let created: serde_json::Value = serde_json::from_slice(&created.stdout).unwrap();
    let id = created["id"].as_str().unwrap();

    let mut worker = Command::new("sleep").arg("30").spawn().unwrap();
    let metadata_path = root.join("sessions").join(id).join("metadata.json");
    let mut metadata: serde_json::Value =
        serde_json::from_slice(&fs::read(&metadata_path).unwrap()).unwrap();
    metadata["status"] = serde_json::json!("running");
    metadata["pid"] = serde_json::json!(worker.id());
    fs::write(
        &metadata_path,
        serde_json::to_vec_pretty(&metadata).unwrap(),
    )
    .unwrap();

    let stopped = run(binary, root, &["kill", id]);
    assert!(
        stopped.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&stopped.stderr),
        String::from_utf8_lossy(&stopped.stdout)
    );
    let stopped: serde_json::Value = serde_json::from_slice(&stopped.stdout).unwrap();
    assert_eq!(stopped["status"], "idle");
    assert!(worker.try_wait().unwrap().is_some());

    let loaded = run(binary, root, &["get", id]);
    assert!(loaded.status.success());
    let loaded: serde_json::Value = serde_json::from_slice(&loaded.stdout).unwrap();
    assert_eq!(loaded["metadata"]["status"], "idle");
    assert_eq!(loaded["metadata"]["pid"], serde_json::Value::Null);
    assert_eq!(
        loaded["metadata"]["termination_reason"],
        "cancelled by user"
    );
    let events = fs::read_to_string(root.join("sessions").join(id).join("events.jsonl")).unwrap();
    assert!(events.lines().any(|line| {
        serde_json::from_str::<serde_json::Value>(line)
            .unwrap()
            .get("type")
            == Some(&serde_json::json!("termination"))
    }));

    let follow_up = run(binary, root, &["send", "--no-run", "--id", id, "follow-up"]);
    assert!(
        follow_up.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&follow_up.stderr),
        String::from_utf8_lossy(&follow_up.stdout)
    );
}
#[cfg(unix)]
fn set_executable(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

#[cfg(not(unix))]
fn set_executable(_: &std::path::Path) {}

#[test]
fn worker_setup_error_fails_the_session_with_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    orchid::config::init(root).unwrap();
    let binary = env!("CARGO_BIN_EXE_orchid");
    fs::write(
        root.join("agents/default.json"),
        r#"{"prompt":"default","connection":"missing"}"#,
    )
    .unwrap();
    let created: serde_json::Value =
        serde_json::from_slice(&run(binary, root, &["create"]).stdout).unwrap();
    let id = created["id"].as_str().unwrap();
    assert!(run(binary, root, &["send", "--id", id, "hi"])
        .status
        .success());
    assert!(run(binary, root, &["await", id, "--timeout", "10"])
        .status
        .success());
    let session: serde_json::Value =
        serde_json::from_slice(&run(binary, root, &["get", id]).stdout).unwrap();
    assert_eq!(session["metadata"]["status"], "failed");
    let reason = session["metadata"]["termination_reason"].as_str().unwrap();
    assert!(reason.contains("connection missing"), "{reason}");
}

#[test]
fn stop_on_idle_session_is_a_no_op_and_working_dirs_are_absolute() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    orchid::config::init(root).unwrap();
    let binary = env!("CARGO_BIN_EXE_orchid");
    assert!(
        !run(binary, root, &["create", "--working-dir", "/no/such/dir"])
            .status
            .success()
    );
    let created = Command::new(binary)
        .current_dir(root)
        .args([
            "--config",
            root.to_str().unwrap(),
            "create",
            "--working-dir",
            "prompts",
        ])
        .output()
        .unwrap();
    let id = serde_json::from_slice::<serde_json::Value>(&created.stdout).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let get = |id: &str| -> serde_json::Value {
        serde_json::from_slice(&run(binary, root, &["get", id]).stdout).unwrap()
    };
    let working_dir = get(&id)["metadata"]["working_dir"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(std::path::Path::new(&working_dir).is_absolute() && working_dir.ends_with("prompts"));
    let stopped = run(binary, root, &["stop", &id]);
    assert!(stopped.status.success());
    assert_eq!(get(&id)["events"].as_array().unwrap().len(), 0);
}
