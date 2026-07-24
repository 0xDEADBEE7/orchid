use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum HookEvent {
    TurnStart,
    TurnStop,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HookConfiguration {
    #[serde(rename = "turn-start", default)]
    pub turn_start: Vec<String>,
    #[serde(rename = "turn-stop", default)]
    pub turn_stop: Vec<String>,
}

impl HookConfiguration {
    pub fn validate(&self) -> Result<(), String> {
        for (event, entries) in [
            (HookEvent::TurnStart, &self.turn_start),
            (HookEvent::TurnStop, &self.turn_stop),
        ] {
            for entry in entries {
                if entry.trim().is_empty() {
                    return Err(format!("{} hook entries must not be empty", event));
                }
            }
        }
        Ok(())
    }
    pub fn paths_for(&self, event: HookEvent, working_dir: &Path) -> Vec<PathBuf> {
        let entries = match event {
            HookEvent::TurnStart => &self.turn_start,
            HookEvent::TurnStop => &self.turn_stop,
        };
        entries
            .iter()
            .map(|entry| {
                let path = Path::new(entry);
                if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    working_dir.join(path)
                }
            })
            .collect()
    }
}

impl std::fmt::Display for HookEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TurnStart => write!(f, "turn-start"),
            Self::TurnStop => write!(f, "turn-stop"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HookStatus {
    Running,
    Succeeded,
    Failed,
    Cancelled,
    BudgetExceeded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HookPayload {
    pub event: HookEvent,
    pub session_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub status: HookStatus,
    pub working_dir: PathBuf,
    pub config_dir: PathBuf,
    pub session_dir: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_typed_event_and_status() {
        let value = serde_json::to_value(HookPayload {
            event: HookEvent::TurnStop,
            session_id: "s1".into(),
            timestamp: chrono::Utc::now(),
            status: HookStatus::Failed,
            working_dir: "/work".into(),
            config_dir: "/config".into(),
            session_dir: "/session".into(),
            error: Some("failed".into()),
            reason: None,
        })
        .unwrap();
        assert_eq!(value["event"], "turn-stop");
        assert_eq!(value["status"], "failed");
        assert_eq!(value["error"], "failed");
        assert!(value.get("reason").is_none());
    }

    #[test]
    fn resolves_relative_paths_from_working_directory() {
        let hooks = HookConfiguration {
            turn_start: vec!["scripts/start.sh".into()],
            turn_stop: vec!["/opt/stop".into()],
        };
        assert_eq!(
            hooks.paths_for(HookEvent::TurnStart, Path::new("/work")),
            vec![PathBuf::from("/work/scripts/start.sh")]
        );
        assert_eq!(
            hooks.paths_for(HookEvent::TurnStop, Path::new("/work")),
            vec![PathBuf::from("/opt/stop")]
        );
    }

    #[test]
    fn rejects_empty_entries() {
        let hooks = HookConfiguration {
            turn_start: vec![" ".into()],
            turn_stop: vec![],
        };
        assert!(hooks.validate().is_err());
    }
}
