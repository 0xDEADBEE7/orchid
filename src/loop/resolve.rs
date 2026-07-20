use crate::types::TokenBudget;

/// Legacy persona resolution is intentionally unavailable.
pub fn resolve_system_prompt(
    _prompt_name: &str,
    _config: &crate::config::ConfigDir,
) -> Result<String, String> {
    Err(
        "legacy persona prompt resolution is unavailable; resolve a policy prompt instead"
            .to_string(),
    )
}

pub fn resolve_persona_budget(
    _persona_name: &str,
    global: &TokenBudget,
    _config: &crate::config::ConfigDir,
) -> TokenBudget {
    global.clone()
}
