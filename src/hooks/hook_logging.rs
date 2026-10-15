//! Provides the hook logging functionality.
use crate::{config::Settings, store::Store};

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
