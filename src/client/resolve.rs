use std::env;

/// Replace all `env.<VAR>` tokens in a string with the corresponding env var value.
/// Handles both whole-value (`env.FOO`) and inline (`Bearer env.FOO`) forms.
pub fn resolve_env_inline(s: &str) -> String {
    resolve_env_inline_strict(s).unwrap_or_else(|_| s.to_string())
}

#[derive(Debug)]
pub struct EnvResolutionError {
    pub variables: Vec<String>,
}

impl std::fmt::Display for EnvResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "unset environment variables: {}",
            self.variables.join(", ")
        )
    }
}

impl std::error::Error for EnvResolutionError {}

pub fn resolve_env_inline_strict(s: &str) -> Result<String, EnvResolutionError> {
    let mut result = String::with_capacity(s.len());
    let mut missing = Vec::new();
    let mut rest = s;
    while let Some(start) = rest.find("env.") {
        result.push_str(&rest[..start]);
        let after = &rest[start + 4..];
        let end = after
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(after.len());
        let var_name = &after[..end];
        match env::var(var_name) {
            Ok(value) => result.push_str(&value),
            Err(_) => {
                missing.push(var_name.to_string());
                result.push_str(&rest[start..start + 4 + end]);
            }
        }
        rest = &after[end..];
    }
    result.push_str(rest);
    missing.sort();
    missing.dedup();
    if missing.is_empty() {
        Ok(result)
    } else {
        Err(EnvResolutionError { variables: missing })
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_env_inline_strict;

    #[test]
    fn reports_unset_names_without_values() {
        std::env::remove_var("ORCHID_TEST_MISSING");
        let error = resolve_env_inline_strict("Bearer env.ORCHID_TEST_MISSING").unwrap_err();
        assert_eq!(error.variables, vec!["ORCHID_TEST_MISSING"]);
        assert!(!error.to_string().contains("Bearer"));
    }

    #[test]
    fn preserves_literals_and_resolves_values() {
        std::env::set_var("ORCHID_TEST_VALUE", "secret");
        assert_eq!(
            resolve_env_inline_strict("Bearer env.ORCHID_TEST_VALUE").unwrap(),
            "Bearer secret"
        );
        assert_eq!(resolve_env_inline_strict("literal").unwrap(), "literal");
        std::env::remove_var("ORCHID_TEST_VALUE");
    }
}
