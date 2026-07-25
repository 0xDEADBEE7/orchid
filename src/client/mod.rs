pub mod base;
pub mod factory;
pub mod openai_codex;
pub mod transport;

pub use base::{
    Client, ClientError, ClientErrorKind, ClientEvent, ClientRequest, Message, ToolDefinition,
};
pub use factory::client_for;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Connection, Credential, ResolvedConnection};
    use serde_json::json;

    fn connection(interface: &str, credential: Option<Credential>) -> ResolvedConnection {
        ResolvedConnection {
            connection: Connection {
                interface: interface.into(),
                base_url: "http://localhost".into(),
                model: "test-model".into(),
                api_key: None,
                auth: None,
                params: Default::default(),
                headers: Default::default(),
            },
            credential,
            params: Default::default(),
            headers: Default::default(),
        }
    }

    #[test]
    fn factory_selects_codex_for_interface_and_oauth() {
        assert!(client_for(connection(
            "codex",
            Some(Credential::Codex {
                access_token: "token".into(),
                account_id: "account".into()
            })
        ))
        .is_ok());
        assert!(client_for(connection("openai", None)).is_err());
    }

    #[test]
    fn codex_sse_maps_text_tool_call_and_done() {
        let input = format!(
            "data: {}\ndata: {}\ndata: [DONE]\n",
            json!({"type":"response.output_text.delta","delta":"hello"}),
            json!({"type":"response.output_item.done","item":{"type":"function_call","call_id":"call-1","name":"bash","arguments": "{}"}})
        );
        let events = crate::client::openai_codex::parse_sse(&input);
        assert!(events.contains(&ClientEvent::TextDelta("hello".into())));
        assert!(events.contains(&ClientEvent::Done));
        assert!(
            matches!(events.get(1), Some(ClientEvent::ToolCall(call)) if call.call_id == "call-1")
        );
    }

    #[test]
    fn codex_sse_maps_extracted_json_payload() {
        let events = crate::client::openai_codex::parse_sse(r#"{"type":"response.completed"}"#);
        assert_eq!(events, vec![ClientEvent::Done]);
    }
}
