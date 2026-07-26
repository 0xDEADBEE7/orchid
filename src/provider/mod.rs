use crate::{
    config::Settings,
    model::{Session, Usage},
};
use serde_json::Value;
use std::io;

mod command;
mod defs;
mod dispatch;
mod lifecycle;
mod sse;
pub use command::run_command as command;
pub use defs::tools as tool_definitions;
pub use lifecycle::{run, run_with_progress};
pub use sse::parse as parse_sse;

pub trait Provider {
    fn reply(&self, prompt: &str, session: &Session) -> io::Result<String>;
    fn stream(&self, prompt: &str, session: &Session) -> io::Result<Vec<StreamEvent>> {
        Ok(vec![StreamEvent::Text(self.reply(prompt, session)?)])
    }
}

fn usage(value: &Value) -> Option<Usage> {
    Some(Usage {
        input: value
            .pointer("/usage/prompt_tokens")
            .or_else(|| value.pointer("/usage/input_tokens"))?
            .as_u64()? as u32,
        output: value
            .pointer("/usage/completion_tokens")
            .or_else(|| value.pointer("/usage/output_tokens"))?
            .as_u64()? as u32,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    Text(String),
    Usage(Usage),
    ToolCall { name: String, input: Value },
    Done,
    Malformed(String),
}

pub struct LocalProvider;
impl Provider for LocalProvider {
    fn reply(&self, prompt: &str, _session: &Session) -> io::Result<String> {
        Ok(format!("local client: {prompt}"))
    }
}

pub struct ClientProvider {
    pub client: Box<dyn crate::client::Client>,
    pub model: String,
    pub system_prompt: String,
    pub tools: Vec<String>,
    pub params: serde_json::Map<String, Value>,
}
impl Provider for ClientProvider {
    fn reply(&self, prompt: &str, session: &Session) -> io::Result<String> {
        let events = self.stream(prompt, session)?;
        Ok(events
            .into_iter()
            .filter_map(|event| {
                if let StreamEvent::Text(text) = event {
                    Some(text)
                } else {
                    None
                }
            })
            .collect())
    }
    fn stream(&self, prompt: &str, session: &Session) -> io::Result<Vec<StreamEvent>> {
        let mut messages = session
            .events
            .iter()
            .filter_map(|event| match event {
                crate::model::Event::Message { role, content, .. } => {
                    Some(crate::client::Message {
                        role: role.clone(),
                        content: content.clone(),
                        tool_calls: Vec::new(),
                        tool_result: None,
                    })
                }
                crate::model::Event::ToolCall { calls, .. } => Some(crate::client::Message {
                    role: "assistant".into(),
                    content: String::new(),
                    tool_calls: calls.clone(),
                    tool_result: None,
                }),
                crate::model::Event::ToolResult {
                    call_id, content, ..
                } => Some(crate::client::Message {
                    role: "tool".into(),
                    content: content.to_string(),
                    tool_calls: Vec::new(),
                    tool_result: Some((call_id.clone(), content.clone())),
                }),
                _ => None,
            })
            .collect::<Vec<_>>();
        messages.push(crate::client::Message {
            role: "user".into(),
            content: prompt.into(),
            tool_calls: Vec::new(),
            tool_result: None,
        });
        let tools = defs::tools(&self.tools)
            .into_iter()
            .filter_map(|tool| {
                Some(crate::client::ToolDefinition {
                    name: tool["function"]["name"].as_str()?.into(),
                    description: tool["function"]["description"].as_str()?.into(),
                    parameters: tool["function"]["parameters"].clone(),
                })
            })
            .collect();
        self.client
            .stream(crate::client::ClientRequest {
                model: self.model.clone(),
                system_prompt: self.system_prompt.clone(),
                messages,
                tools,
                params: self.params.clone(),
            })
            .map_err(|error| io::Error::other(error.to_string()))
            .map(|events| {
                events
                    .into_iter()
                    .filter_map(|event| match event {
                        crate::client::ClientEvent::TextDelta(text) => {
                            Some(StreamEvent::Text(text))
                        }
                        crate::client::ClientEvent::ToolCall(call) => Some(StreamEvent::ToolCall {
                            name: call.name,
                            input: call.input,
                        }),
                        crate::client::ClientEvent::Usage(usage) => Some(StreamEvent::Usage(usage)),
                        crate::client::ClientEvent::Done => Some(StreamEvent::Done),
                        crate::client::ClientEvent::ReasoningDelta(_) => None,
                    })
                    .collect()
            })
    }
}
pub fn tool_allowed(settings: &Settings, name: &str) -> bool {
    settings.policy.tools().iter().any(|tool| tool == name)
}
pub fn tool_result(value: impl Into<Value>) -> Value {
    value.into()
}
