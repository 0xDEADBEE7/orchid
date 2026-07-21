use std::collections::HashMap;
use std::env;
use std::fs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedConnection {
    pub interface: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub params: HashMap<String, serde_json::Value>,
    pub headers: HashMap<String, String>,
}

pub fn resolve_auth(profile: &crate::config::AuthProfile) -> Result<String, EnvResolutionError> {
    let reference = profile.value.as_deref().ok_or_else(|| EnvResolutionError {
        variables: vec!["missing credential reference".into()],
    })?;
    let value = if let Some(name) = reference.strip_prefix("env.") {
        env::var(name).map_err(|_| EnvResolutionError {
            variables: vec![name.to_string()],
        })?
    } else if let Some(path) = reference.strip_prefix("file.") {
        fs::read_to_string(path).map_err(|_| EnvResolutionError {
            variables: vec![path.to_string()],
        })?
    } else {
        return Err(EnvResolutionError {
            variables: vec!["invalid authentication reference".into()],
        });
    };
    let value = value.strip_suffix('\n').unwrap_or(&value).to_string();
    if value.is_empty() {
        return Err(EnvResolutionError {
            variables: vec![reference.to_string()],
        });
    }
    Ok(value)
}

pub fn resolve_connection(
    connection: &crate::config::Connection,
) -> Result<ResolvedConnection, EnvResolutionError> {
    let api_key = connection
        .api_key
        .as_deref()
        .map(resolve_env_inline_strict)
        .transpose()?;
    let headers = connection
        .headers
        .iter()
        .map(|(name, value)| resolve_env_inline_strict(value).map(|value| (name.clone(), value)))
        .collect::<Result<HashMap<_, _>, _>>()?;
    Ok(ResolvedConnection {
        interface: connection.interface.clone(),
        base_url: connection.base_url.clone(),
        api_key,
        model: connection.model.clone(),
        params: connection.params.clone(),
        headers,
    })
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
    use super::{resolve_auth, resolve_env_inline_strict};
    use crate::config::AuthProfile;

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

    #[test]
    fn resolves_file_reference_and_trims_one_final_newline() {
        let path = std::env::temp_dir().join("orchid-auth-test-key");
        std::fs::write(&path, b"file-secret\n").unwrap();
        let profile = AuthProfile {
            kind: "api_key".into(),
            value: Some(format!("file.{}", path.display())),
        };
        assert_eq!(resolve_auth(&profile).unwrap(), "file-secret");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_empty_environment_credentials() {
        std::env::set_var("ORCHID_EMPTY_AUTH", "");
        let profile = AuthProfile {
            kind: "api_key".into(),
            value: Some("env.ORCHID_EMPTY_AUTH".into()),
        };
        assert!(resolve_auth(&profile).is_err());
        std::env::remove_var("ORCHID_EMPTY_AUTH");
    }
}
