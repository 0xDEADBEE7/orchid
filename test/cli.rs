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
fn tool_call_persists_result_and_hook_can_invoke_it() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    orchid::config::init(root).unwrap();
    let binary = env!("CARGO_BIN_EXE_orchid");
    fs::write(
        root.join("policies/default.json"),
        r#"{"tools":["bash"],"hooks":{"events":{"on-init":[{"script":"hook.sh","mode":"sync"}]}}}"#,
    )
    .unwrap();
    let hook = format!(
        "#!/bin/sh\n{} --config {} tool-call --id \"$ORCHID_SESSION_ID\" --input '{{\"call_id\":\"hook-call\",\"name\":\"bash\",\"input\":{{\"cmd\":\"printf hook > hook-output\"}}}}'\n",
        shell_quote(binary),
        shell_quote(root.to_str().unwrap()),
    );
    fs::write(root.join("hook.sh"), hook).unwrap();
    set_executable(&root.join("hook.sh"));

    let created = run(binary, root, &["create"]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let created: serde_json::Value = serde_json::from_slice(&created.stdout).unwrap();
    let id = created["id"].as_str().unwrap();

    let direct = run(
        binary,
        root,
        &[
            "tool-call",
            "--id",
            id,
            "--input",
            r#"{"call_id":"direct-call","name":"bash","input":{"cmd":"printf direct > direct-output"}}"#,
        ],
    );
    assert!(
        direct.status.success(),
        "{}",
        String::from_utf8_lossy(&direct.stderr)
    );
    assert!(direct.stdout.is_empty());
    assert_eq!(
        fs::read_to_string(root.join("direct-output")).unwrap(),
        "direct"
    );
    let direct_events =
        fs::read_to_string(root.join("sessions").join(id).join("events.jsonl")).unwrap();
    assert!(direct_events
        .lines()
        .any(|line| line.contains("direct-call")));

    let hooked = run(binary, root, &["create"]);
    assert!(hooked.status.success());
    let hooked: serde_json::Value = serde_json::from_slice(&hooked.stdout).unwrap();
    let hooked_id = hooked["id"].as_str().unwrap();
    let sent = run(
        binary,
        root,
        &["send", "--no-run", "--id", hooked_id, "start"],
    );
    assert!(
        sent.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&sent.stderr),
        String::from_utf8_lossy(&sent.stdout)
    );
    assert_eq!(
        fs::read_to_string(root.join("hook-output")).unwrap(),
        "hook"
    );

    let events =
        fs::read_to_string(root.join("sessions").join(hooked_id).join("events.jsonl")).unwrap();
    let values: Vec<serde_json::Value> = events
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(values
        .iter()
        .any(|event| event["type"] == "tool_call" && event["calls"][0]["call_id"] == "hook-call"));
    assert!(values
        .iter()
        .any(|event| event["type"] == "tool_result" && event["call_id"] == "hook-call"));
}

#[test]
fn session_metadata_keeps_an_independent_agent_policy_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    orchid::config::init(root).unwrap();
    let binary = env!("CARGO_BIN_EXE_orchid");

    fs::write(
        root.join("policies/default.json"),
        r#"{"max_tokens":120000,"tools":["bash"]}"#,
    )
    .unwrap();
    let created = run(binary, root, &["create"]);
    assert!(created.status.success());
    let created: serde_json::Value = serde_json::from_slice(&created.stdout).unwrap();
    let id = created["id"].as_str().unwrap();

    fs::write(
        root.join("policies/default.json"),
        r#"{"max_tokens":4096,"tools":[]}"#,
    )
    .unwrap();
    let loaded = run(binary, root, &["get", id]);
    assert!(loaded.status.success());
    let loaded: serde_json::Value = serde_json::from_slice(&loaded.stdout).unwrap();
    assert_eq!(loaded["metadata"]["policy"]["max_tokens"], 120000);
    assert_eq!(loaded["metadata"]["policy"]["tools"][0], "bash");
    assert!(loaded["metadata"].get("agent").is_none());
    fs::write(
        root.join("prompts/default.txt"),
        "A changed source prompt.",
    )
    .unwrap();
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
    assert_eq!(sessions[0], serde_json::json!({"id": first_id, "label": "first"}));
    assert_eq!(sessions[1], serde_json::json!({"id": second_id, "label": null}));
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
    fs::write(&metadata_path, serde_json::to_vec_pretty(&metadata).unwrap()).unwrap();

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
    assert_eq!(loaded["metadata"]["termination_reason"], "cancelled by user");
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

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
