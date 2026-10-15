//! Provides the mod functionality.
use crate::model::{Session, Usage};
use serde_json::Value;
use std::io;

mod command;
mod defs;
pub mod dispatch;
mod lifecycle;
pub use command::run_command as command;
pub use defs::tools as tool_definitions;
pub use lifecycle::{run, run_with_progress};

pub trait Provider {
    /// Requests the next reply for the session's transcript.
    fn stream(&self, session: &Session) -> io::Result<Vec<StreamEvent>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    Text(String),
    Usage(Usage),
    ToolCall { name: String, input: Value },
    Done,
}

/// Performs the ClientProvider operation.
pub struct ClientProvider {
    pub client: Box<dyn crate::client::Client>,
    pub model: String,
    pub system_prompt: String,
    pub tools: Vec<String>,
    pub params: serde_json::Map<String, Value>,
}
impl Provider for ClientProvider {
    /// Streams events for a request.
    fn stream(&self, session: &Session) -> io::Result<Vec<StreamEvent>> {
        let messages = messages(session);
        self.client
            .stream(request(self, messages))
            .map_err(io::Error::other)
            .map(stream_events)
    }
}

/// Performs the messages operation.
pub(crate) fn messages(session: &Session) -> Vec<crate::client::Message> {
    session
        .events
        .iter()
        .filter_map(|event| match event {
            crate::model::Event::Message { role, content, .. } => Some(crate::client::Message {
                role: role.clone(),
                content: content.clone(),
                tool_calls: Vec::new(),
                tool_call_id: None,
            }),
            crate::model::Event::ToolCall { calls, .. } => Some(crate::client::Message {
                role: "assistant".into(),
                content: String::new(),
                tool_calls: calls.clone(),
                tool_call_id: None,
            }),
            crate::model::Event::ToolResult {
                call_id, content, ..
            } => Some(crate::client::Message {
                role: "tool".into(),
                content: crate::tools::text(&crate::tools::clip(content.clone())),
                tool_calls: Vec::new(),
                tool_call_id: Some(call_id.clone()),
            }),
            _ => None,
        })
        .collect()
}

/// Builds and validates a request.
fn request(
    provider: &ClientProvider,
    messages: Vec<crate::client::Message>,
) -> crate::client::ClientRequest {
    let tools = defs::tools(&provider.tools)
        .into_iter()
        .filter_map(|tool| {
            Some(crate::client::ToolDefinition {
                name: tool["function"]["name"].as_str()?.into(),
                description: tool["function"]["description"].as_str()?.into(),
                parameters: tool["function"]["parameters"].clone(),
            })
        })
        .collect();
    crate::client::ClientRequest {
        model: provider.model.clone(),
        system_prompt: provider.system_prompt.clone(),
        messages,
        tools,
        params: provider.params.clone(),
    }
}

/// Performs the stream events operation.
fn stream_events(events: Vec<crate::client::ClientEvent>) -> Vec<StreamEvent> {
    events
        .into_iter()
        .filter_map(|event| match event {
            crate::client::ClientEvent::TextDelta(text) => Some(StreamEvent::Text(text)),
            crate::client::ClientEvent::ToolCall(call) => Some(StreamEvent::ToolCall {
                name: call.name,
                input: call.input,
            }),
            crate::client::ClientEvent::Usage(usage) => Some(StreamEvent::Usage(usage)),
            crate::client::ClientEvent::Done => Some(StreamEvent::Done),
            crate::client::ClientEvent::ReasoningDelta(_) => None,
        })
        .collect()
}
