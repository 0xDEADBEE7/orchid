use crate::config::resolve::create_provider_from_connection;
use crate::cmd::create::resolve_working_dir;
use crate::config::resolve::{resolve as resolve_effective_config, EffectiveSessionConfig};
use crate::config::ConfigDir;
use crate::convo::{resolve, Store};
use crate::log::LogWriter;
use crate::loop_module::run as run_tool_loop;
use crate::types::{ConvoEvent, MessageEvent};
use crate::{get_convo_jsonl_path, get_orchid_dir};
use serde_json::json;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::process::Stdio;

pub fn send(
    id: Option<String>,
    message: String,
    await_completion: bool,
    config_dir: &std::path::Path,
    label: Option<String>,
    working_dir: Option<String>,
) -> Result<serde_json::Value, String> {
    let store = Store::new()?;

    let convo_id = if let Some(id_or_label) = id {
        let base_path = get_orchid_dir()?.join("conversations");
        let resolved_id = resolve::resolve(&id_or_label, &base_path)?.id;

        if label.is_some() || working_dir.is_some() {
            let mut updates = crate::MetadataUpdate::default();
            if let Some(l) = label {
                updates.label = Some(Some(l));
            }
            if let Some(wd) = working_dir {
                updates.working_dir = Some(Some(wd));
            }
            store.update(&resolved_id, updates)?;
        }

        resolved_id
    } else {
        let wd = resolve_working_dir(working_dir)?;
        let meta = store.create(label, Some(wd), None, None, None)?;
        meta.id
    };

    let meta = store.get(&convo_id)?;
    let _working_dir = meta
        .working_dir
        .clone()
        .ok_or_else(|| "conversation has no working directory configured".to_string())?;

    if meta.status == crate::types::Status::Running {
        return Err(format!("conversation {} is already running", convo_id));
    }

    let convo_path = get_convo_jsonl_path(&convo_id)?;
    let event = ConvoEvent::Message(MessageEvent::new("user", &message));
    LogWriter::append(&convo_path, &event)?;

    let config_dir_ref = ConfigDir::new(config_dir);
    let effective = resolve_effective_config(&config_dir_ref, None, Some(&_working_dir))
        .map_err(|e| format!("failed to resolve effective config: {}", e))?;

    if await_completion {
        let connection = effective
            .connection_candidates
            .first()
            .ok_or_else(|| "policy has no connections configured".to_string())?;

        let log_path = get_orchid_dir()
            .ok()
            .map(|d| d.join("conversations").join(&convo_id).join("orchid.log"));

        let provider =
            create_provider_from_connection(connection, log_path).map_err(|e| e.to_string())?;

        run_tool_loop(&convo_id, &effective, config_dir, provider.as_ref())?;

        let final_meta = store.get(&convo_id)?;
        Ok(json!({
            "id": convo_id,
            "status": final_meta.status,
            "completed": true,
            "policy": effective.policy_name,
        }))
    } else {
        fork_tool_loop(&convo_id, &effective, config_dir)
    }
}

fn fork_tool_loop(
    convo_id: &str,
    effective: &EffectiveSessionConfig,
    config_dir: &std::path::Path,
) -> Result<serde_json::Value, String> {
    let temp_dir = std::env::temp_dir();
    let effective_path = temp_dir.join(format!("orchid-effective-{}.json", convo_id));
    let json_str = serde_json::to_string_pretty(effective)
        .map_err(|e| format!("failed to serialize effective config: {}", e))?;
    std::fs::write(&effective_path, &json_str)
        .map_err(|e| format!("failed to write effective config: {}", e))?;

    let mut cmd = std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    cmd.arg("__run")
        .arg(convo_id)
        .arg("--effective-config")
        .arg(effective_path.display().to_string())
        .arg("--config")
        .arg(config_dir.display().to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            nix::unistd::setsid().map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;
            Ok(())
        });
    }

    let child = cmd
        .spawn()
        .map_err(|e| format!("failed to spawn background process: {}", e))?;

    let pid = child.id();

    let _ = std::fs::remove_file(&effective_path);

    let updates = crate::MetadataUpdate {
        pid: Some(Some(pid)),
        ..Default::default()
    };

    Store::new()?.update(convo_id, updates)?;

    Ok(json!({
        "id": convo_id,
        "status": "running",
        "pid": pid,
        "policy": effective.policy_name,
    }))
}
