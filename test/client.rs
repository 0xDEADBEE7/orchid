use orchid::client::{client_for, Client, ClientError, ClientEvent, ClientRequest};
use orchid::config::{Connection, Credential, ResolvedConnection};
use orchid::model::Session;
use orchid::provider::{ClientProvider, Provider};
use serde_json::json;
use std::sync::{Arc, Mutex};

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
fn factory_selects_echo_and_returns_the_latest_message() {
    let client = client_for(connection("echo", None)).unwrap();
    let events = client
        .stream(orchid::client::ClientRequest {
            model: "ignored".into(),
            system_prompt: "ignored".into(),
            messages: vec![orchid::client::Message {
                role: "user".into(),
                content: "hello".into(),
                tool_calls: Vec::new(),
                tool_result: None,
            }],
            tools: Vec::new(),
            params: Default::default(),
        })
        .unwrap();
    assert_eq!(
        events,
        vec![ClientEvent::TextDelta("hello".into()), ClientEvent::Done]
    );
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
    assert!(client_for(connection("openai", None)).is_ok());
    assert!(client_for(connection("local", None)).is_ok());
}

#[test]
fn openai_chat_sse_maps_text_usage_tool_call_and_done() {
    let first = json!({"choices":[{"delta":{"content":"hello","tool_calls":[{"index":0,"id":"call-1","function":{"name":"bash","arguments":"{\"cmd\":"}}]}}]});
    let second = json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"pwd\"}"}}]}}],"usage":{"prompt_tokens":12,"completion_tokens":3}});
    let input = format!("data: {first}\ndata: {second}\ndata: [DONE]\n");
    let events = orchid::client::openai_chat::parse_sse(&input).unwrap();
    assert!(events.contains(&ClientEvent::TextDelta("hello".into())));
    assert!(events.contains(&ClientEvent::Usage(orchid::model::Usage {
        input: 12,
        output: 3,
        cached_input: 0,
    })));
    assert!(matches!(
        events.iter().find(|event| matches!(event, ClientEvent::ToolCall(_))),
        Some(ClientEvent::ToolCall(call)) if call.call_id == "call-1" && call.name == "bash" && call.input == json!({"cmd":"pwd"})
    ));
    assert_eq!(events.last(), Some(&ClientEvent::Done));
}

struct CapturingClient(Arc<Mutex<Vec<ClientRequest>>>);

impl Client for CapturingClient {
    fn stream(&self, request: ClientRequest) -> Result<Vec<ClientEvent>, ClientError> {
        self.0.lock().unwrap().push(request);
        Ok(vec![ClientEvent::Done])
    }
}

#[test]
fn configured_provider_sends_persisted_user_message_once() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = ClientProvider {
        client: Box::new(CapturingClient(requests.clone())),
        model: "test-model".into(),
        system_prompt: "system".into(),
        tools: Vec::new(),
        params: Default::default(),
    };
    let mut session = Session::new(None, None, None);
    session.append(Session::message("user", "only once".into()));

    provider.stream("only once", &session).unwrap();

    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].messages.len(), 1);
    assert_eq!(requests[0].messages[0].role, "user");
    assert_eq!(requests[0].messages[0].content, "only once");
}

#[test]
fn codex_sse_maps_text_tool_call_and_done() {
    let input = format!(
        "data: {}\ndata: {}\ndata: [DONE]\n",
        json!({"type":"response.output_text.delta","delta":"hello"}),
        json!({"type":"response.output_item.done","item":{"type":"function_call","call_id":"call-1","name":"bash","arguments":"{}"}})
    );
    let events = orchid::client::openai_codex::parse_sse(&input);
    assert!(events.contains(&ClientEvent::TextDelta("hello".into())));
    assert!(events.contains(&ClientEvent::Done));
    assert!(matches!(events.get(1), Some(ClientEvent::ToolCall(call)) if call.call_id == "call-1"));
}

#[test]
fn codex_sse_maps_extracted_json_payload() {
    assert_eq!(
        orchid::client::openai_codex::parse_sse(
            r#"{"type":"response.completed","response":{"usage":{"input_tokens":123,"output_tokens":45,"input_tokens_details":{"cached_tokens":100}}}}"#
        ),
        vec![
            ClientEvent::Usage(orchid::model::Usage {
                input: 123,
                output: 45,
                cached_input: 100,
            }),
            ClientEvent::Done,
        ]
    );
}
