use crate::session::get_session_jsonl_path;
pub use crate::types::TokenBudget;

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

/// Estimate token usage by dividing conversation JSONL byte length by 3.
/// This is vendor-agnostic and conservative enough to be reliable at scale.
pub fn check(convo_id: &str, budget: &TokenBudget) -> BudgetStatus {
    let total = estimate_tokens(convo_id).unwrap_or(0);

    if total >= budget.hard_limit {
        BudgetStatus::Exceeded { total }
    } else if total >= budget.warn_threshold {
        BudgetStatus::Warning { total }
    } else {
        BudgetStatus::Ok { total }
    }
}

fn estimate_tokens(convo_id: &str) -> Option<u32> {
    let path = get_session_jsonl_path(convo_id).ok()?;
    let bytes = std::fs::metadata(&path).ok()?.len();
    Some((bytes / 3) as u32)
}
