use crate::{
    config::{HookMode, Settings},
    store::Store,
};

pub(super) fn format_mode(mode: &HookMode) -> &'static str {
    match mode {
        HookMode::Sync => "sync",
        HookMode::Async => "async",
    }
}

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
        &crate::model::LogRecord {
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            level: level.into(),
            message: message.into(),
            fields,
        },
        &settings.log_level,
    );
}
