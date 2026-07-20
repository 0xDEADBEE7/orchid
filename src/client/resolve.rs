use std::env;

/// Replace all `env.<VAR>` tokens in a string with the corresponding env var value.
/// Handles both whole-value (`env.FOO`) and inline (`Bearer env.FOO`) forms.
pub fn resolve_env_inline(s: &str) -> String {
    let mut result = s.to_string();
    while let Some(start) = result.find("env.") {
        let after = &result[start + 4..];
        let end = after
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(after.len());
        let var_name = &after[..end];
        let value = env::var(var_name).unwrap_or_default();
        result = format!("{}{}{}", &result[..start], value, &after[end..]);
    }
    result
}
