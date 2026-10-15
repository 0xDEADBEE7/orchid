use orchid::client::{client_for, Client, ClientError, ClientEvent, ClientRequest};
use orchid::config::{Connection, Credential, ResolvedConnection};
use orchid::model::Session;
use orchid::provider::{ClientProvider, Provider};
use serde_json::json;
use std::sync::{Arc, Mutex};

fn codex_http_response(status: &str, body: &str) -> (ClientError, String) {
    use std::io::{BufRead, BufReader, Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let response = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nX-Request-Id: request-test\r\nConnection: close\r\n\r\n{body}", body.len());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(&stream);
        let mut headers = String::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            headers.push_str(&line);
        }
        let size: usize = headers
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length: ")
                    .map(str::to_owned)
            })
            .unwrap()
            .parse()
            .unwrap();
        reader.read_exact(&mut vec![0; size]).unwrap();
        stream.write_all(response.as_bytes()).unwrap();
        headers
    });
    let mut config = connection(
        "codex",
        Some(Credential::Codex {
            access_token: "test-token".into(),
            account_id: "test-account".into(),
        }),
    );
    config.connection.base_url = format!("http://{address}");
    let error = client_for(config)
        .unwrap()
        .stream(ClientRequest {
            model: "gpt-6-astra".into(),
            system_prompt: "smoke test".into(),
            messages: vec![],
            tools: vec![],
            params: Default::default(),
        })
        .unwrap_err();
    (error, server.join().unwrap())
}

#[test]
fn codex_sends_current_headers_and_preserves_gateway_error() {
    let (error, headers) = codex_http_response(
        "400 Bad Request",
        r#"{"detail":"model requires a newer version of Codex"}"#,
    );
    let headers = headers.to_ascii_lowercase();
    assert!(headers.contains("version: 0.153.4\r\n"));
    assert!(headers.contains("user-agent: codex_cli_rs/0.153.4 (orchid)\r\n"));
    let message = error.to_string();
    assert!(message.contains("400"));
    assert!(message.contains("request-test"));
    assert!(message.contains("model requires a newer version"));
}

#[test]
fn codex_bounds_error_body_and_classifies_authentication() {
    let (error, _) = codex_http_response("401 Unauthorized", &"x".repeat(10000));
    assert_eq!(error.kind, orchid::client::ClientErrorKind::Authentication);
    assert!(error.to_string().len() < 4300);
}

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
