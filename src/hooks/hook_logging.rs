//! Provides the hook logging functionality.
use crate::{
    config::{HookMode, Settings},
    store::Store,
};

/// Performs the format mode operation.
pub(super) fn format_mode(mode: &HookMode) -> &'static str {
    match mode {
        HookMode::Sync => "sync",
        HookMode::Async => "async",
    }
}

/// Performs the log lifecycle operation.
pub(super) fn log_lifecycle(
    settings: &Settings,
    session_id: &str,
    message: &str,
    level: &str,
    fields: serde_json::Value,
) {
    let Ok(store) = Store::new(&settings.root) else {
        return;
    };
    let _ = store.log_both(
        session_id,
        &crate::model::LogRecord::new(level, message, fields),
        &settings.log_level,
    );
}
