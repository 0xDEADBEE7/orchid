use crate::types::SessionEvent;
use chrono::Utc;
use serde_json::json;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

// ── Session JSONL ────────────────────────────────────────────────────────

pub struct LogWriter;

impl LogWriter {
    pub fn append<P: AsRef<Path>>(path: P, event: &SessionEvent) -> Result<String, String> {
        let entry = serde_json::to_string(event)
            .map_err(|e| format!("failed to serialize event: {}", e))?;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| format!("failed to open log file: {}", e))?;

        writeln!(file, "{}", entry).map_err(|e| format!("failed to write log entry: {}", e))?;

        let event_id = match event {
            SessionEvent::Message(e) => e.event_id.clone(),
            SessionEvent::ToolCall(e) => e.event_id.clone(),
            SessionEvent::ToolResult(e) => e.event_id.clone(),
            SessionEvent::Reasoning(e) => e.event_id.clone(),
        };

        Ok(event_id)
    }
}

pub struct LogReader;

impl LogReader {
    pub fn read_lines<P: AsRef<Path>>(path: P) -> Result<Vec<SessionEvent>, String> {
        let file =
            std::fs::File::open(path).map_err(|e| format!("failed to open log file: {}", e))?;

        let reader = BufReader::new(file);
        let mut events = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(|e| format!("failed to read line: {}", e))?;

            if line.trim().is_empty() {
                continue;
            }

            let event: SessionEvent =
                serde_json::from_str(&line).map_err(|e| format!("failed to parse JSON: {}", e))?;

            events.push(event);
        }

        Ok(events)
    }
}

// ── Diagnostic logger ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn from_config_str(s: Option<&str>) -> Self {
        match s {
            Some("debug") => LogLevel::Debug,
            _ => LogLevel::Info,
        }
    }
}

/// Runtime diagnostic logger. Writes newline-delimited JSON to `orchid.log`
/// inside the session directory. Silently no-ops if the path is unset.
pub struct DiagLogger {
    path: Option<PathBuf>,
    level: LogLevel,
}

impl DiagLogger {
    /// Logger that writes to `<session_dir>/orchid.log`.
    pub fn for_session(session_dir: PathBuf, level: LogLevel) -> Self {
        DiagLogger {
            path: Some(session_dir.join("orchid.log")),
            level,
        }
    }

    /// No-op logger (e.g. outside a session context).
    pub fn noop() -> Self {
        DiagLogger {
            path: None,
            level: LogLevel::Info,
        }
    }

    pub fn debug(&self, event: &str, detail: &str) {
        if self.level <= LogLevel::Debug {
            self.write("DEBUG", event, detail);
        }
    }

    pub fn info(&self, event: &str, detail: &str) {
        if self.level <= LogLevel::Info {
            self.write("INFO", event, detail);
        }
    }

    pub fn warn(&self, event: &str, detail: &str) {
        if self.level <= LogLevel::Warn {
            self.write("WARN", event, detail);
        }
    }

    pub fn error(&self, event: &str, detail: &str) {
        self.write("ERROR", event, detail);
    }

    fn write(&self, level: &str, event: &str, detail: &str) {
        let Some(ref path) = self.path else { return };

        let entry = json!({
            "ts": Utc::now(),
            "level": level,
            "event": event,
            "detail": detail,
        });

        // Best-effort — diagnostic failures must not crash the main loop.
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "{}", entry);
        }
    }
}
