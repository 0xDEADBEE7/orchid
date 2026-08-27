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
    pub tool_result: Option<(String, Value)>,
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
    Transport,
}
impl ClientError {
    /// Creates a new value.
    pub fn new(kind: ClientErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
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
