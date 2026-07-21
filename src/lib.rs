pub mod cli;
pub mod client;
pub mod cmd;
pub mod config;
pub mod jsonerr;
pub mod log;
pub mod r#loop;
pub mod provider;
pub mod session;
pub mod tools;
pub mod types;
pub mod loop_module {
    pub use crate::r#loop::run::run;
}

pub use cli::{parse_args, Command, ConfigSubcommand};
pub use client::base::{is_retryable, BaseClient};
pub use client::{create_provider_from_connection_with_log, resolve_env_inline_strict};
pub use cmd::{config_list, config_show, config_use, config_validate, delete, internal_run, list, send, set};
pub use config::resolve::{
    create_provider_from_connection, create_provider_from_connections_with_log,
    resolve as resolve_effective_config, EffectiveSessionConfig,
};
pub use config::{
    ConfigDir, Connection, Permissions, Policy, PolicyLimits, ResourceLoadError, RootConfig,
};
pub use jsonerr::JsonError;
pub use log::{DiagLogger, LogReader, LogWriter};
pub use provider::{Provider, ProviderError, Response, StreamEvent};
pub use r#loop::history::{build_message_history, replace_stale_in_value};
pub use session::{
    generate_id, get_session_jsonl_path, is_id_format, resolve, SessionStore, SessionUpdate,
};
pub use tools::{execute_tool, tool_definitions, Tool};
pub use types::{Message, Metadata, SessionState, Status, TokenBudget, ToolCall, ToolResult};
