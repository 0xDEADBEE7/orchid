use crate::model::{ToolCall, Usage};
use serde::Serialize;
use serde_json::{Map, Value};
use std::{fmt, io};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub tool_result: Option<(String, Value)>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}
#[derive(Debug, Clone, PartialEq)]
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
    Transport,
}
impl ClientError {
    pub fn new(kind: ClientErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}
impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(f)
    }
}
impl std::error::Error for ClientError {}
impl From<io::Error> for ClientError {
    fn from(e: io::Error) -> Self {
        Self::new(ClientErrorKind::Transport, e.to_string())
    }
}
pub trait Client: Send + Sync {
    fn stream(&self, request: ClientRequest) -> Result<Vec<ClientEvent>, ClientError>;
}
