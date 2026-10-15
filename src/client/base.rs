//! Provides the base functionality.
use crate::model::{ToolCall, Usage};
use serde::Serialize;
use serde_json::{Map, Value};
use std::{fmt, io};

#[derive(Debug, Clone, PartialEq, Serialize)]
/// Performs the Message operation.
pub struct Message {
    pub role: String,
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    /// Set on tool results: the call this message answers.
    pub tool_call_id: Option<String>,
}
#[derive(Debug, Clone, PartialEq)]
/// Performs the ToolDefinition operation.
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}
#[derive(Debug, Clone, PartialEq)]
/// Performs the ClientRequest operation.
pub struct ClientRequest {
    pub model: String,
    pub system_prompt: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDefinition>,
    pub params: Map<String, Value>,
}
#[derive(Debug, Clone, PartialEq)]
pub enum ClientEvent {
    TextDelta(String),
    ReasoningDelta(String),
    ToolCall(ToolCall),
    Usage(Usage),
    Done,
}
#[derive(Debug)]
/// Performs the ClientError operation.
pub struct ClientError {
    pub kind: ClientErrorKind,
    message: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientErrorKind {
    Configuration,
    Authentication,
    Http,
    Protocol,
    /// Network failure: connect, DNS, reset, or an interrupted stream.
    Transport,
    /// The server reported a transient condition (overload, 5xx, rate limit).
    Unavailable,
}
impl ClientError {
    /// Creates a new value.
    pub fn new(kind: ClientErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// Whether repeating the same request may succeed.
    pub fn retryable(&self) -> bool {
        matches!(
            self.kind,
            ClientErrorKind::Transport | ClientErrorKind::Unavailable
        )
    }
}

/// Classifies an HTTP status. 408, 429 and 5xx are transient, except a 429 for an
/// exhausted quota and 501/505, which repeat identically.
pub fn status_kind(status: u16, body: &str) -> ClientErrorKind {
    match status {
        401 | 403 => ClientErrorKind::Authentication,
        429 if body.contains("usage_limit_reached") => ClientErrorKind::Http,
        408 | 429 => ClientErrorKind::Unavailable,
        501 | 505 => ClientErrorKind::Http,
        500..=599 => ClientErrorKind::Unavailable,
        _ => ClientErrorKind::Http,
    }
}

/// Classifies an error object reported inside a response stream by its code.
pub fn error_kind(error: &Value) -> ClientErrorKind {
    match &error["code"] {
        Value::Number(code) => status_kind(code.as_u64().unwrap_or(0) as u16, ""),
        Value::String(code)
            if matches!(
                code.as_str(),
                "server_error" | "server_is_overloaded" | "rate_limit_exceeded"
            ) =>
        {
            ClientErrorKind::Unavailable
        }
        _ => ClientErrorKind::Http,
    }
}

/// Builds an error from a failed HTTP response, keeping a bounded body and the
/// gateway's request ID.
pub fn http_error(label: &str, response: reqwest::blocking::Response) -> ClientError {
    use std::io::Read;
    let status = response.status().as_u16();
    let request_id = response
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("missing")
        .to_owned();
    let mut bytes = Vec::new();
    let detail = match response.take(4096).read_to_end(&mut bytes) {
        Ok(_) => String::from_utf8_lossy(&bytes).trim().to_owned(),
        Err(_) => "<unable to read error body>".to_owned(),
    };
    ClientError::new(
        status_kind(status, &detail),
        format!("{label} HTTP status {status} (request_id={request_id}): {detail}"),
    )
}
impl fmt::Display for ClientError {
    /// Formats the value for display.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(f)
    }
}
impl std::error::Error for ClientError {}
impl From<io::Error> for ClientError {
    /// Converts the source value into this type.
    fn from(e: io::Error) -> Self {
        Self::new(ClientErrorKind::Transport, e.to_string())
    }
}
pub trait Client: Send + Sync {
    /// Streams events for a request.
    fn stream(&self, request: ClientRequest) -> Result<Vec<ClientEvent>, ClientError>;
}
