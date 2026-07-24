use crate::config::resolve::EffectiveSessionConfig;
use crate::log::{DiagLogger, LogLevel};
use crate::provider::Provider;
use crate::r#loop::guard::RunGuard;
use crate::r#loop::lifecycle;
use crate::r#loop::stream::{self, StreamOutcome};
use crate::r#loop::{events, history};
use crate::session::get_session_dir_from_config;
use crate::tools;
use crate::types::{Status, TokenBudget, ToolResult};
use globset::GlobSet;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Context gathered during the setup phase, passed into the main loop.
pub struct LoopContext {
    pub store: crate::session::SessionStore,
    pub meta: crate::types::Metadata,
    pub session_dir: PathBuf,
    pub log: DiagLogger,
    pub config_dir: PathBuf,
    pub working_dir: String,
    pub hooks: crate::config::HookConfiguration,
    pub permissions: crate::config::Permissions,
    pub limits: crate::config::PolicyLimits,
    pub prompt: String,
    pub env_vars: HashMap<String, String>,
    pub warn_interval: u32,
    pub global_scope_set: GlobSet,
    pub session_scope_set: GlobSet,
}

/// Build the loop context from an `EffectiveSessionConfig` and session ID.
pub fn build_context(
    session_id: &str,
    effective: &EffectiveSessionConfig,
    config_dir: &Path,
) -> Result<LoopContext, String> {
    let store = crate::session::SessionStore::with_config_dir(config_dir)?;
    let meta = store.get(session_id)?;

    let session_paths = store.state(session_id)?.restrictions;
    let permissions = crate::config::resolve::intersect_permissions(
        &effective.permissions,
        session_paths.as_deref(),
    );
    if session_paths.is_some()
        && permissions.paths.is_empty()
        && !effective.permissions.paths.is_empty()
    {
        return Err("session path restrictions do not overlap policy permissions".to_string());
    }

    let session_dir = get_session_dir_from_config(session_id, config_dir)?;
    let log_level = LogLevel::Info;
    let log = DiagLogger::for_session(session_dir.clone(), log_level);

    if lifecycle::detect_crashed(session_id, config_dir)? {
        let state = store.state(session_id)?;
        let stale_pid = state
            .pid
            .map(|p| p.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        log.info(
            "run_crashed",
            &format!("pid={} stale — reconciling", stale_pid),
        );
        lifecycle::reconcile_crashed(session_id, config_dir)?;
        lifecycle::run_hook(
            &effective.hooks,
            crate::config::HookEvent::TurnStop,
            lifecycle::hook_payload(
                crate::config::HookEvent::TurnStop,
                session_id,
                crate::config::HookStatus::Failed,
                &effective.working_dir,
                config_dir,
                &session_dir,
                Some("previous run was detected as crashed".into()),
                Some("crash reconciliation".into()),
            ),
            &log,
        );
    }

    log.info("run_start", session_id);
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

    lifecycle::on_run_start(session_id, config_dir)?;

    lifecycle::run_hook(
        &effective.hooks,
        crate::config::HookEvent::TurnStart,
        lifecycle::hook_payload(
            crate::config::HookEvent::TurnStart,
            session_id,
            crate::config::HookStatus::Running,
            &effective.working_dir,
            config_dir,
            &session_dir,
            None,
            None,
        ),
        &log,
    );

    let working_dir = effective.working_dir.to_string_lossy().to_string();

    // Compute warn interval from limits (same logic as before).
    let warn_interval = effective
        .limits
        .token_warn_threshold
        .zip(effective.limits.token_hard_limit)
        .map(|(warn, hard)| (hard.saturating_sub(warn)) / 10)
        .unwrap_or(4_000); // default: 4000 (from 40k warn / 80k hard)

    let global_scope_set = GlobSet::empty();
    // Session restrictions are intersected with policy permissions above;
    // they must never become path-escape exceptions.
    let session_scope_set = GlobSet::empty();

    Ok(LoopContext {
        store,
        meta,
        session_dir,
        log,
        config_dir: config_dir.to_path_buf(),
        working_dir,
        hooks: effective.hooks.clone(),
        permissions,
        limits: effective.limits.clone(),
        prompt: effective.prompt.clone(),
        env_vars: effective.env_vars.clone(),
        warn_interval,
        global_scope_set,
        session_scope_set,
    })
}

enum LoopOutcome {
    ContinueWithTools(crate::provider::Response),
    Complete(crate::provider::Response),
    Empty,
    Failed(String),
}

enum ResponseDecision {
    Continue,
    Complete,
}

fn provider_turn(
    ctx: &LoopContext,
    provider: &dyn Provider,
    messages: Vec<crate::types::Message>,
) -> LoopOutcome {
    ctx.log
        .info("provider_send", &format!("messages={}", messages.len()));
    let event_iter = match provider.send_streaming(ctx.prompt.clone(), messages) {
        Ok(events) => events,
        Err(e) => {
            ctx.log.error("provider_error", &e.to_string());
            return LoopOutcome::Failed(format!("provider error: {}", e));
        }
    };
    match stream::reduce_response_stream(&ctx.log, &ctx.session_dir, event_iter) {
        Ok(StreamOutcome::ContinueWithTools(response)) => LoopOutcome::ContinueWithTools(response),
        Ok(StreamOutcome::Complete(response)) => LoopOutcome::Complete(response),
        Ok(StreamOutcome::Empty) => LoopOutcome::Empty,
        Err(error) => LoopOutcome::Failed(error),
    }
}

/// Execute the main session loop.
pub fn run_loop(ctx: &mut LoopContext, provider: &dyn Provider) -> Result<(), String> {
    let mut guard = RunGuard::with_hooks(
        &ctx.meta.id,
        &ctx.config_dir,
        Path::new(&ctx.working_dir),
        &ctx.session_dir,
        ctx.hooks.clone(),
        &ctx.log,
    );
    let mut last_warn_tokens: Option<u32> = None;

    loop {
        let messages = history::build_message_history(&ctx.meta.id, &ctx.config_dir, &ctx.log)?;

        let estimated_tokens = history::estimate_tokens_from_messages(&messages);
        let hard_limit = ctx.limits.token_hard_limit.unwrap_or(120_000);
        let warn_threshold = ctx.limits.token_warn_threshold.unwrap_or(80_000);

        if estimated_tokens >= hard_limit {
            return terminate_for_budget(ctx, &mut guard, estimated_tokens, hard_limit, true);
        }

        let response = match provider_turn(ctx, provider, messages) {
            LoopOutcome::ContinueWithTools(response) | LoopOutcome::Complete(response) => response,
            LoopOutcome::Empty => empty_response(),
            LoopOutcome::Failed(error) => return Err(error),
        };

        if let Some(ref u) = response.usage {
            ctx.log.info(
                "usage",
                &format!("in={} out={}", u.input_tokens, u.output_tokens),
            );
        }

        {
            let updates = crate::session::SessionUpdate {
                token_estimate: Some(estimated_tokens),
                ..Default::default()
            };
            ctx.store.update(&ctx.meta.id, updates)?;
        }

        if estimated_tokens >= hard_limit {
            return terminate_for_budget(ctx, &mut guard, estimated_tokens, hard_limit, false);
        } else if estimated_tokens >= warn_threshold {
            let should_warn = should_warn_for_budget(
                estimated_tokens,
                warn_threshold,
                last_warn_tokens,
                ctx.warn_interval,
            );
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
                    &ctx.config_dir,
                    &format!(
                        "[WARNING] This session has consumed {} tokens (warn threshold: {}). \
                        Consider wrapping up or the session will be terminated at {} tokens.",
                        estimated_tokens, warn_threshold, hard_limit
                    ),
                )?;
            }
        }

        if matches!(handle_response(ctx, response)?, ResponseDecision::Complete) {
            break;
        }
    }

    finish_loop(ctx, &mut guard, Status::Idle, None, None, &ctx.meta.id)?;
    Ok(())
}

fn should_warn_for_budget(
    estimated_tokens: u32,
    warn_threshold: u32,
    last_warn_tokens: Option<u32>,
    warn_interval: u32,
) -> bool {
    estimated_tokens >= warn_threshold
        && last_warn_tokens
            .is_none_or(|last| estimated_tokens >= last.saturating_add(warn_interval))
}

fn empty_response() -> crate::provider::Response {
    crate::provider::Response {
        message: None,
        reasoning: None,
        tool_calls: None,
        usage: None,
        model: None,
    }
}

fn handle_response(
    ctx: &LoopContext,
    response: crate::provider::Response,
) -> Result<ResponseDecision, String> {
    if response.tool_calls.is_some() {
        execute_tool_turn(ctx, response)?;
        return Ok(ResponseDecision::Continue);
    }

    if let Some(message) = response.message {
        if !message.trim().is_empty() {
            ctx.log.info("run_complete", "");
            events::append_message(&ctx.meta.id, &ctx.config_dir, &message)?;
            if let Some(ref reasoning) = response.reasoning {
                events::append_reasoning(&ctx.meta.id, &ctx.config_dir, reasoning)?;
            }
            ctx.store.update(
                &ctx.meta.id,
                crate::session::SessionUpdate {
                    last_message: Some(message),
                    ..Default::default()
                },
            )?;
            return Ok(ResponseDecision::Complete);
        }
    }

    ctx.log.warn("empty_response", "");
    let empty_msg = "The previous response contained no text and no tool calls. Please respond with a message or use a tool.";
    events::append_system(&ctx.meta.id, &ctx.config_dir, empty_msg)?;
    Ok(ResponseDecision::Continue)
}

fn finish_loop(
    ctx: &LoopContext,
    guard: &mut RunGuard<'_>,
    status: Status,
    error: Option<String>,
    reason: Option<String>,
    log_message: &str,
) -> Result<(), String> {
    guard.finish(status, error, reason)?;
    ctx.log.info("run_end", log_message);
    Ok(())
}

fn terminate_for_budget(
    ctx: &LoopContext,
    guard: &mut RunGuard<'_>,
    estimated_tokens: u32,
    hard_limit: u32,
    before_send: bool,
) -> Result<(), String> {
    let (termination_msg, error) = if before_send {
        ctx.log.info(
            "pre_send_budget_exceeded",
            &format!("estimated={} hard_limit={}", estimated_tokens, hard_limit),
        );
        (
            format!(
                "[SESSION TERMINATED] Estimated token count ({}) would exceed hard limit ({}) before sending. \\
                Start a new session to continue.",
                estimated_tokens, hard_limit
            ),
            format!(
                "token hard limit would be exceeded before sending: {} estimated tokens",
                estimated_tokens
            ),
        )
    } else {
        ctx.log.warn(
            "token_budget_exceeded",
            &format!("total={} hard_limit={}", estimated_tokens, hard_limit),
        );
        (
            format!(
                "[SESSION TERMINATED] Token hard limit reached ({} / {} tokens). \\
                The run has been stopped. Start a new session to continue.",
                estimated_tokens, hard_limit
            ),
            format!("token hard limit exceeded: {} tokens", estimated_tokens),
        )
    };
    events::append_system(&ctx.meta.id, &ctx.config_dir, &termination_msg)?;
    ctx.store.update(
        &ctx.meta.id,
        crate::session::SessionUpdate {
            last_message: Some(termination_msg),
            token_estimate: before_send.then_some(estimated_tokens),
            ..Default::default()
        },
    )?;
    finish_loop(
        ctx,
        guard,
        Status::Failed,
        Some("token budget termination".into()),
        Some("budget termination".into()),
        if before_send {
            "pre_send_budget_exceeded"
        } else {
            "budget_exceeded"
        },
    )?;
    Err(error)
}

fn execute_tool_turn(ctx: &LoopContext, response: crate::provider::Response) -> Result<(), String> {
    let Some(tool_calls) = response.tool_calls else {
        return Ok(());
    };

    if let Some(ref message) = response.message {
        if !message.trim().is_empty() {
            events::append_message(&ctx.meta.id, &ctx.config_dir, message)?;
        }
    }

    for tool_call in tool_calls {
        ctx.log.info(
            "tool_call",
            &format!("tool={} id={}", tool_call.name, tool_call.id),
        );
        events::append_tool_call(
            &ctx.meta.id,
            &ctx.config_dir,
            std::slice::from_ref(&tool_call),
        )?;

        let content = match tools::execute_tool_with_permissions(
            &tool_call.name,
            tool_call.input.clone(),
            &tools::ToolContext {
                working_dir: &ctx.working_dir,
                env_vars: &ctx.env_vars,
                global_scope_set: &ctx.global_scope_set,
                session_scope_set: &ctx.session_scope_set,
                allowed_tools: &ctx.permissions.tools,
                allowed_paths: &ctx.permissions.paths,
            },
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

        events::append_tool_result(
            &ctx.meta.id,
            &ctx.config_dir,
            &ToolResult {
                call_id: tool_call.id,
                content,
            },
        )?;
    }

    Ok(())
}

/// Top-level entry point: setup + loop.
pub fn run(
    session_id: &str,
    effective: &EffectiveSessionConfig,
    config_dir: &Path,
    provider: &dyn Provider,
) -> Result<(), String> {
    let mut ctx = build_context(session_id, effective, config_dir)?;
    run_loop(&mut ctx, provider)?;
    Ok(())
}

/// Build context with a custom budget (used by tests).
pub fn build_context_with_budget(
    session_id: &str,
    effective: &EffectiveSessionConfig,
    config_dir: &Path,
    budget_override: &TokenBudget,
) -> Result<LoopContext, String> {
    let mut ctx = build_context(session_id, effective, config_dir)?;
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
