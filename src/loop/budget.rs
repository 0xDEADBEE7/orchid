pub use crate::types::TokenBudget;
use std::path::Path;

pub enum BudgetStatus {
    Ok { total: u32 },
    Warning { total: u32 },
    Exceeded { total: u32 },
}

impl BudgetStatus {
    pub fn total(&self) -> u32 {
        match self {
            BudgetStatus::Ok { total } => *total,
            BudgetStatus::Warning { total } => *total,
            BudgetStatus::Exceeded { total } => *total,
        }
    }
}

/// Estimate token usage by dividing session JSONL byte length by 3.
/// This is vendor-agnostic and conservative enough to be reliable at scale.
pub fn check(session_id: &str, budget: &TokenBudget) -> BudgetStatus {
    let total = estimate_tokens(session_id).unwrap_or(0);

    if total >= budget.hard_limit {
        BudgetStatus::Exceeded { total }
    } else if total >= budget.warn_threshold {
        BudgetStatus::Warning { total }
    } else {
        BudgetStatus::Ok { total }
    }
}

fn estimate_tokens(session_id: &str) -> Option<u32> {
    let base_path = std::env::var_os("ORCHID_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| Path::new("config").to_path_buf())
        .join("sessions")
        .join(session_id);
    let legacy_path = std::env::var_os("ORCHID_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| Path::new("config").to_path_buf())
        .join("conversations")
        .join(session_id)
        .join("conversation.jsonl");
    let path = if base_path.join("conversation.jsonl").exists() {
        base_path.join("conversation.jsonl")
    } else if base_path.join("session.jsonl").exists() {
        base_path.join("session.jsonl")
    } else {
        legacy_path
    };
    let bytes = std::fs::metadata(path).ok()?.len();
    Some((bytes / 3) as u32)
}
