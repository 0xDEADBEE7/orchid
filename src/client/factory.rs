use super::openai_chat::OpenAiChatClient;
use super::openai_codex::CodexClient;
use super::{Client, ClientError, ClientErrorKind};
use crate::config::ResolvedConnection;

pub fn client_for(connection: ResolvedConnection) -> Result<Box<dyn Client>, ClientError> {
    let interface = connection.connection.interface.as_str();
    let codex_auth = matches!(
        connection.credential,
        Some(crate::config::Credential::Codex { .. })
    );
    match (interface, codex_auth) {
        ("echo", _) => Ok(Box::new(super::EchoClient)),
        ("local", _) | ("openai", false) => Ok(Box::new(OpenAiChatClient::new(connection))),
        ("codex" | "openai-codex", _) | ("openai", true) => {
            Ok(Box::new(CodexClient::new(connection)))
        }
        _ => Err(ClientError::new(
            ClientErrorKind::Configuration,
            "unsupported client interface",
        )),
    }
}
