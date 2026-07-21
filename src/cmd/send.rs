use crate::cmd::create::resolve_working_dir;
use crate::config::resolve::{
    create_provider_from_connections_with_log, resolve_with_prompt as resolve_effective_config,
    EffectiveSessionConfig,
};
use crate::config::ConfigDir;
use crate::log::LogWriter;
use crate::loop_module::run as run_tool_loop;
use crate::session::{resolve, SessionStore};
use crate::types::{MessageEvent, SessionEvent};
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
    policy: Option<String>,
    prompt: Option<String>,
) -> Result<serde_json::Value, String> {
    let store = SessionStore::with_config_dir(config_dir)?;

    let new_session = id.is_none();
    let (session_id, _meta, effective) = if let Some(id_or_label) = id {
        let base_path = config_dir.join("sessions");
        let resolved_id = resolve::resolve(&id_or_label, &base_path)?.id;

        if label.is_some() || working_dir.is_some() {
            let mut updates = crate::SessionUpdate::default();
            if let Some(l) = label {
                updates.label = Some(Some(l));
            }
            if let Some(wd) = working_dir {
                updates.working_dir = Some(Some(wd));
            }
            store.update(&resolved_id, updates)?;
        }

        let meta = store.get(&resolved_id)?;
        let working_dir = meta
            .working_dir
            .clone()
            .ok_or_else(|| "session has no working directory configured".to_string())?;
        let effective = resolve_effective_config(
            &ConfigDir::new(config_dir),
            meta.policy.as_deref().or(policy.as_deref()),
            prompt.as_deref(), Some(&working_dir),
        )
        .map_err(|e| format!("failed to resolve effective config: {}", e))?;
        let meta = if policy.is_some() || prompt.is_some() {
            store.update(
                &resolved_id,
                crate::SessionUpdate {
                    policy: Some(Some(effective.policy_name.clone())),
                    policy_hash: Some(Some(effective.policy_hash.clone())),
                    prompt: Some(effective.prompt_name.clone()),
                    ..Default::default()
                },
            )?
        } else {
            meta
        };
        (resolved_id, meta, effective)
    } else {
        let wd = resolve_working_dir(working_dir)?;
        let effective =
            resolve_effective_config(&ConfigDir::new(config_dir), policy.as_deref(), prompt.as_deref(), Some(&wd))
                .map_err(|e| format!("failed to resolve effective config: {}", e))?;
        if await_completion {
            create_provider_from_connections_with_log(&effective.connection_candidates, None)
                .map_err(|e| e.to_string())?;
        }
        let meta = store.create(label, Some(wd), None)?;
        let meta = store.update(
            &meta.id,
            crate::SessionUpdate {
                policy: Some(Some(effective.policy_name.clone())),
                policy_hash: Some(Some(effective.policy_hash.clone())),
                prompt: Some(effective.prompt_name.clone()),
                ..Default::default()
            },
        )?;
        (meta.id.clone(), meta, effective)
    };

    let state = store.state(&session_id)?;
    if state.status == crate::types::Status::Running {
        return Err(format!("session {} is already running", session_id));
    }

    let log_path = if new_session {
        None
    } else {
        Some(
            config_dir
                .join("sessions")
                .join(&session_id)
                .join("orchid.log"),
        )
    };
    let provider = if await_completion {
        Some(
            create_provider_from_connections_with_log(&effective.connection_candidates, log_path)
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };

    let session_path = store.transcript_path(&session_id);
    let event = SessionEvent::Message(MessageEvent::new("user", &message));
    LogWriter::append(&session_path, &event)?;

    if await_completion {
        run_tool_loop(
            &session_id,
            &effective,
            config_dir,
            provider
                .as_ref()
                .expect("provider created for await")
                .as_ref(),
        )?;

        let _final_meta = store.get(&session_id)?;
        Ok(json!({
            "id": session_id,
            "status": store.state(&session_id)?.status,
            "completed": true,
            "policy": effective.policy_name,
        }))
    } else {
        fork_tool_loop(&session_id, &effective, config_dir)
    }
}

fn fork_tool_loop(
    session_id: &str,
    effective: &EffectiveSessionConfig,
    config_dir: &std::path::Path,
) -> Result<serde_json::Value, String> {
    let mut cmd = std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    cmd.arg("__run")
        .arg(session_id)
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

    let updates = crate::SessionUpdate {
        pid: Some(Some(pid)),
        ..Default::default()
    };

    SessionStore::with_config_dir(config_dir)?.update(session_id, updates)?;

    Ok(json!({
        "id": session_id,
        "status": "running",
        "pid": pid,
        "policy": effective.policy_name,
    }))
}
