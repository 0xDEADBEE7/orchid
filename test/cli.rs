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
