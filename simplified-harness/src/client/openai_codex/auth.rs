use super::{ClientError, ClientErrorKind};
use crate::config::Credential;

pub struct CodexAuth;

impl CodexAuth {
    pub fn present(credential: &Credential) -> Result<(&str, &str), ClientError> {
        match credential {
            Credential::Codex {
                access_token,
                account_id,
            } if !access_token.is_empty() => Ok((access_token, account_id)),
            _ => Err(ClientError::new(
                ClientErrorKind::Authentication,
                "Codex OAuth credential required",
            )),
        }
    }
}
