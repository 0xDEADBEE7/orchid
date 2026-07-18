use crate::cmd::create::resolve_working_dir;
use crate::convo::{resolve, Store};
use crate::log::LogWriter;
use crate::loop_module::run as run_tool_loop;
use crate::types::{ConvoEvent, MessageEvent};
use crate::{get_convo_jsonl_path, get_orchid_dir, load_config};
use crate::client::create_provider_with_log;
use serde_json::json;
use std::process::Stdio;
#[cfg(unix)]
use std::os::unix::process::CommandExt;

pub fn send(
    id: Option<String>,
    message: String,
    await_completion: bool,
    profile: Option<String>,
    label: Option<String>,
    working_dir: Option<String>,
) -> Result<serde_json::Value, String> {
    let store = Store::new()?;
    let config = load_config()?;
    let active_profile = config.current_profile.clone();

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
    let _working_dir = meta.working_dir.clone().ok_or_else(|| {
        "conversation has no working directory configured".to_string()
    })?;

    if meta.status == crate::types::Status::Running {
        return Err(format!("conversation {} is already running", convo_id));
    }

    let convo_path = get_convo_jsonl_path(&convo_id)?;
    let event = ConvoEvent::Message(MessageEvent::new("user", &message));
    LogWriter::append(&convo_path, &event)?;

    if await_completion {
        let profile_name =
            profile.unwrap_or(active_profile.ok_or_else(|| "no profile configured".to_string())?);

        let profiles = config.profiles;
        let prof = profiles
            .get(&profile_name)
            .ok_or_else(|| format!("profile '{}' not found", profile_name))?;

        let log_level = crate::log::LogLevel::from_config_str(config.log_level.as_deref());
        let convo_dir = get_orchid_dir()?.join("conversations").join(&convo_id);
        let log = crate::log::DiagLogger::for_convo(convo_dir.clone(), log_level);

        log.debug("profile_selected", &profile_name);
        log.debug("profile_base_url", &prof.base_url);
        log.debug("profile_model", &prof.model);
        log.debug(
            "profile_api_key",
            if prof.api_key.is_empty() { "(empty)" } else { "(set)" },
        );
        log.debug(
            "profile_headers",
            &prof
                .headers
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
        );

        let provider = create_provider_with_log(prof, Some(convo_dir.join("orchid.log"))).map_err(|e| {
            log.error("provider_init_error", &e.to_string());
            format!("provider error: {}", e)
        })?;
        log.debug("provider_init", "ok");

        run_tool_loop(&convo_id, provider.as_ref())?;

        let final_meta = store.get(&convo_id)?;
        Ok(json!({
            "id": convo_id,
            "status": final_meta.status,
            "completed": true
        }))
    } else {
        let pid = fork_tool_loop(&convo_id, &profile, active_profile)?;

        let updates = crate::MetadataUpdate {
            pid: Some(pid),
            ..Default::default()
        };

        store.update(&convo_id, updates)?;

        Ok(json!({
            "id": convo_id,
            "status": "running",
            "pid": pid
        }))
    }
}

fn fork_tool_loop(
    convo_id: &str,
    profile: &Option<String>,
    active_profile: Option<String>,
) -> Result<Option<u32>, String> {
    let profile_arg = profile
        .clone()
        .or(active_profile)
        .ok_or_else(|| "no profile configured and no --profile given".to_string())?;

    let mut cmd =
        std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    cmd.arg("__run")
        .arg(convo_id)
        .arg("--profile")
        .arg(profile_arg)
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    // Detach from the caller's process group and controlling terminal so the
    // daemon survives when the parent process exits (e.g. when spawned via
    // Emacs make-process which places children in its own process group).
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

    Ok(Some(child.id()))
}


