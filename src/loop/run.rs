use crate::config::resolve::EffectiveSessionConfig;
use crate::convo::get_convo_dir_from_config;
use crate::log::{DiagLogger, LogLevel};
use crate::provider::{Provider, StreamEvent};
use crate::r#loop::guard::RunGuard;
use crate::r#loop::lifecycle;
use crate::r#loop::stream::StreamState;
use crate::r#loop::{events, history};
use crate::tools;
use crate::types::{TokenBudget, ToolResult};
use globset::GlobSet;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Context gathered during the setup phase, passed into the main loop.
pub struct LoopContext {
    pub store: crate::convo::Store,
    pub meta: crate::types::Metadata,
    pub convo_dir: PathBuf,
    pub log: DiagLogger,
    pub config_dir: PathBuf,
    pub working_dir: String,
    pub permissions: crate::config::Permissions,
    pub limits: crate::config::PolicyLimits,
    pub env_vars: HashMap<String, String>,
    pub prompt: String,
    pub warn_interval: u32,
    pub global_scope_set: GlobSet,
    pub convo_scope_set: GlobSet,
}

/// Build the loop context from an `EffectiveSessionConfig` and conversation ID.
/// This is the new-path that uses resource-based config instead of legacy profiles.
pub fn build_context(
    convo_id: &str,
    effective: &EffectiveSessionConfig,
    config_dir: &Path,
) -> Result<LoopContext, String> {
    let store = crate::convo::Store::with_config_dir(config_dir)?;
    let meta = store.get(convo_id)?;

    let convo_dir = get_convo_dir_from_config(convo_id, config_dir)?;
    let log_level = LogLevel::Info;
    let log = DiagLogger::for_convo(convo_dir.clone(), log_level);

    if lifecycle::detect_crashed(convo_id, config_dir)? {
        let stale_pid = meta
            .pid
            .map(|p| p.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        log.info(
            "run_crashed",
            &format!("pid={} stale — reconciling", stale_pid),
        );
        lifecycle::reconcile_crashed(convo_id, config_dir)?;
    }

    log.info("run_start", convo_id);
    log.info(
        "policy_selected",
        &format!(
            "name={} hash={}",
            effective.policy_name, effective.policy_hash
        ),
    );
    log.info(
        "connection_selected",
        &format!("candidates={}", effective.connection_candidates.len()),
    );

    lifecycle::on_run_start(convo_id, config_dir)?;

    let working_dir = effective.working_dir.to_string_lossy().to_string();

    // Compute warn interval from limits (same logic as before).
    let warn_interval = effective
        .limits
        .token_warn_threshold
        .zip(effective.limits.token_hard_limit)
        .map(|(warn, hard)| (hard.saturating_sub(warn)) / 10)
        .unwrap_or(4_000); // default: 4000 (from 40k warn / 80k hard)

    let global_scope_set = GlobSet::empty();
    let convo_scope_set = GlobSet::empty();

    Ok(LoopContext {
        store,
        meta,
        convo_dir,
        log,
        config_dir: config_dir.to_path_buf(),
        working_dir,
        permissions: effective.permissions.clone(),
        limits: effective.limits.clone(),
        env_vars: effective.env_vars.clone(),
        prompt: effective.prompt.clone(),
        warn_interval,
        global_scope_set,
        convo_scope_set,
    })
}

/// Execute the main conversation loop.
pub fn run_loop(ctx: &mut LoopContext, provider: &dyn Provider) -> Result<(), String> {
    let mut guard = RunGuard::new(&ctx.meta.id, &ctx.config_dir);
    let mut last_warn_tokens: Option<u32> = None;

    loop {
        let messages = history::build_message_history(&ctx.meta.id, &ctx.config_dir, &ctx.log)?;

        let estimated_tokens = history::estimate_tokens_from_messages(&messages);
        let hard_limit = ctx.limits.token_hard_limit.unwrap_or(120_000);
        let warn_threshold = ctx.limits.token_warn_threshold.unwrap_or(80_000);

        if estimated_tokens >= hard_limit {
            ctx.log.info(
                "pre_send_budget_exceeded",
                &format!("estimated={} hard_limit={}", estimated_tokens, hard_limit),
            );
            let termination_msg = format!(
                "[SESSION TERMINATED] Estimated token count ({}) would exceed hard limit ({}) before sending. \
                Start a new conversation to continue.",
                estimated_tokens, hard_limit
            );
            events::append_system(&ctx.meta.id, &termination_msg)?;
            let updates = crate::convo::MetadataUpdate {
                last_message: Some(termination_msg.clone()),
                token_estimate: Some(estimated_tokens),
                ..Default::default()
            };
            ctx.store.update(&ctx.meta.id, updates)?;
            guard.disarm();
            lifecycle::on_run_end(&ctx.meta.id, &ctx.config_dir)?;
            ctx.log.info("run_end", "pre_send_budget_exceeded");
            return Err(format!(
                "token hard limit would be exceeded before sending: {} estimated tokens",
                estimated_tokens
            ));
        }

        ctx.log
            .info("provider_send", &format!("messages={}", messages.len()));

        let mut stream_state = StreamState::create(&ctx.convo_dir);
        let response = {
            let event_iter = provider
                .send_streaming(ctx.prompt.clone(), messages)
                .map_err(|e| {
                    ctx.log.error("provider_error", &e.to_string());
                    format!("provider error: {}", e)
                })?;

            let mut result = None;
            for event in event_iter {
                match event {
                    Err(e) => {
                        ctx.log.error("stream_error", &e.to_string());
                        return Err(format!("provider error: {}", e));
                    }
                    Ok(StreamEvent::TextDelta(_))
                    | Ok(StreamEvent::ToolCallDelta { .. })
                    | Ok(StreamEvent::ReasoningDelta(_)) => {
                        stream_state.tick();
                    }
                    Ok(StreamEvent::Complete(resp)) => {
                        result = Some(resp);
                        break;
                    }
                }
            }
            result.ok_or_else(|| "stream ended without a Complete event".to_string())?
        };

        if let Some(ref u) = response.usage {
            ctx.log.info(
                "usage",
                &format!("in={} out={}", u.input_tokens, u.output_tokens),
            );
        }

        {
            let updates = crate::convo::MetadataUpdate {
                token_estimate: Some(estimated_tokens),
                ..Default::default()
            };
            ctx.store.update(&ctx.meta.id, updates)?;
        }

        if estimated_tokens >= hard_limit {
            ctx.log.warn(
                "token_budget_exceeded",
                &format!("total={} hard_limit={}", estimated_tokens, hard_limit),
            );
            let termination_msg = format!(
                "[SESSION TERMINATED] Token hard limit reached ({} / {} tokens). \
                The run has been stopped. Start a new conversation to continue.",
                estimated_tokens, hard_limit
            );
            events::append_system(&ctx.meta.id, &termination_msg)?;
            let updates = crate::convo::MetadataUpdate {
                last_message: Some(termination_msg),
                ..Default::default()
            };
            ctx.store.update(&ctx.meta.id, updates)?;
            guard.disarm();
            lifecycle::on_run_end(&ctx.meta.id, &ctx.config_dir)?;
            ctx.log.info("run_end", "budget_exceeded");
            return Err(format!(
                "token hard limit exceeded: {} tokens",
                estimated_tokens
            ));
        } else if estimated_tokens >= warn_threshold {
            let should_warn = match last_warn_tokens {
                None => true,
                Some(last) => estimated_tokens >= last.saturating_add(ctx.warn_interval),
            };
            if should_warn {
                last_warn_tokens = Some(estimated_tokens);
                ctx.log.warn(
                    "token_budget_warning",
                    &format!(
                        "total={} warn_threshold={}",
                        estimated_tokens, warn_threshold
                    ),
                );
                events::append_system(
                    &ctx.meta.id,
                    &format!(
                        "[WARNING] This session has consumed {} tokens (warn threshold: {}). \
                        Consider wrapping up or the session will be terminated at {} tokens.",
                        estimated_tokens, warn_threshold, hard_limit
                    ),
                )?;
            }
        }

        if let Some(tool_calls) = response.tool_calls {
            if let Some(ref msg) = response.message {
                if !msg.trim().is_empty() {
                    events::append_message(&ctx.meta.id, msg)?;
                }
            }

            for tool_call in tool_calls {
                ctx.log.info(
                    "tool_call",
                    &format!("tool={} id={}", tool_call.name, tool_call.id),
                );
                events::append_tool_call(&ctx.meta.id, std::slice::from_ref(&tool_call))?;

                let content = match tools::execute_tool(
                    &tool_call.name,
                    tool_call.input.clone(),
                    &ctx.working_dir,
                    false, // scope enforcement is now governed by policy permissions, not per-session
                    &ctx.env_vars,
                    &ctx.global_scope_set,
                    &ctx.convo_scope_set,
                ) {
                    Ok(raw) => {
                        ctx.log
                            .info("tool_result", &format!("tool={}", tool_call.name));
                        raw
                    }
                    Err(e) => {
                        ctx.log
                            .error("tool_error", &format!("tool={} err={}", tool_call.name, e));
                        serde_json::Value::String(format!("Error: {}", e))
                    }
                };

                let tool_result = ToolResult {
                    call_id: tool_call.id,
                    content,
                };

                events::append_tool_result(&ctx.meta.id, &tool_result)?;
            }
        } else if let Some(message) = response.message {
            if message.trim().is_empty() {
                ctx.log.warn("empty_response", "");
                let empty_msg = "The previous response contained no text and no tool calls. Please respond with a message or use a tool.".to_string();
                events::append_system(&ctx.meta.id, &empty_msg)?;
            } else {
                ctx.log.info("run_complete", "");
                events::append_message(&ctx.meta.id, &message)?;

                if let Some(ref reasoning) = response.reasoning {
                    events::append_reasoning(&ctx.meta.id, reasoning)?;
                }

                let updates = crate::convo::MetadataUpdate {
                    last_message: Some(message),
                    ..Default::default()
                };
                ctx.store.update(&ctx.meta.id, updates)?;

                break;
            }
        } else {
            ctx.log.warn("empty_response", "");
            let empty_msg = "The previous response contained no text and no tool calls. Please respond with a message or use a tool.".to_string();
            events::append_system(&ctx.meta.id, &empty_msg)?;
        }
    }

    lifecycle::on_run_end(&ctx.meta.id, &ctx.config_dir)?;
    guard.disarm();
    ctx.log.info("run_end", &ctx.meta.id);
    Ok(())
}

/// Top-level entry point: setup + loop.
pub fn run(
    convo_id: &str,
    effective: &EffectiveSessionConfig,
    config_dir: &Path,
    provider: &dyn Provider,
) -> Result<(), String> {
    let mut ctx = build_context(convo_id, effective, config_dir)?;
    run_loop(&mut ctx, provider)?;
    Ok(())
}

/// Build context with a custom budget (used by tests).
pub fn build_context_with_budget(
    convo_id: &str,
    effective: &EffectiveSessionConfig,
    config_dir: &Path,
    budget_override: &TokenBudget,
) -> Result<LoopContext, String> {
    let mut ctx = build_context(convo_id, effective, config_dir)?;
    // Override limits with the test budget.
    ctx.limits = crate::config::PolicyLimits {
        token_warn_threshold: Some(budget_override.warn_threshold),
        token_hard_limit: Some(budget_override.hard_limit),
        max_steps: None,
    };
    ctx.warn_interval = (budget_override
        .hard_limit
        .saturating_sub(budget_override.warn_threshold))
        / 10;
    Ok(ctx)
}
