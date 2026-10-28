//! Provides the model functionality.
use crate::agent::Agent;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// `hookrunning` was a transient state in earlier versions.
    #[serde(alias = "hookrunning")]
    Idle,
    Running,
    Failed,
    Cancelled,
    Terminated,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Metadata {
    pub id: String,
    pub label: Option<String>,
    pub working_dir: Option<String>,
    pub created_at: DateTime<Utc>,
    /// The session that created this one (e.g. an orchestrator), if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// The agent this session was created from, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
    /// The session's own copy of its agent (connection, tools, cap, hooks).
    pub agent: Agent,
    /// Pre-0.2 policy copy, kept untouched so the migration can be reversed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<Value>,
    pub status: Status,
    pub pid: Option<u32>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub token_estimate: u32,
    #[serde(default)]
    pub token_usage: TokenUsage,
    pub termination_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TokenUsage {
    #[serde(alias = "context_estimate")]
    pub marginal_input: u32,
    #[serde(alias = "input_total")]
    pub cumulative_input: u64,
    #[serde(alias = "output_total")]
    pub cumulative_output: u64,
    #[serde(alias = "cached_input_total")]
    pub cumulative_cached_input: u64,
    pub requests: u64,
    pub method: String,
}

/// A transcript event: the kind-specific fields plus common ones.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    #[serde(flatten)]
    pub kind: EventKind,
    pub event_id: String,
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub connection: Option<String>,
    #[serde(default)]
    pub token_usage: TokenUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventKind {
    Message {
        role: String,
        content: String,
    },
    ToolCall {
        calls: Vec<ToolCall>,
    },
    ToolResult {
        call_id: String,
        content: Value,
    },
    Termination {
        reason: String,
    },
    Failure {
        message: String,
    },
    /// Event types from earlier versions (e.g. `usage`): kept on disk, ignored.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub call_id: String,
    pub name: String,
    pub input: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LogRecord {
    pub event_id: String,
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub message: String,
    #[serde(default)]
    pub fields: Value,
}

impl LogRecord {
    /// Creates a record stamped now.
    pub fn new(level: &str, message: &str, fields: Value) -> Self {
        Self {
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            level: level.into(),
            message: message.into(),
            fields,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Usage {
    pub input: u32,
    pub output: u32,
    #[serde(default)]
    pub cached_input: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub metadata: Metadata,
    pub events: Vec<Event>,
}

impl Event {
    /// Creates an event stamped now.
    pub fn new(kind: EventKind) -> Self {
        Self {
            kind,
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            connection: None,
            token_usage: TokenUsage::default(),
        }
    }
}

impl Session {
    /// Performs the pending tool call ids operation.
    pub fn pending_tool_call_ids(&self) -> Vec<String> {
        let completed = self
            .events
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::ToolResult { call_id, .. } => Some(call_id.as_str()),
                _ => None,
            })
            .collect::<HashSet<_>>();
        self.events
            .iter()
            .flat_map(|event| match &event.kind {
                EventKind::ToolCall { calls } => calls
                    .iter()
                    .filter(|call| !completed.contains(call.call_id.as_str()))
                    .map(|call| call.call_id.clone())
                    .collect::<Vec<_>>(),
                _ => Vec::new(),
            })
            .collect()
    }

    /// Returns the latest assistant message.
    pub fn last_message(&self) -> Option<&str> {
        self.events
            .iter()
            .rev()
            .find_map(|event| match &event.kind {
                EventKind::Message { role, content } if role == "assistant" => {
                    Some(content.as_str())
                }
                _ => None,
            })
    }

    /// Answers every unanswered tool call with a terminated result, so the
    /// transcript stays valid for the next request.
    pub fn close_pending(&mut self, error: &str) {
        for call_id in self.pending_tool_call_ids() {
            let content = serde_json::json!({"status": "terminated", "error": error});
            self.append(Event::new(EventKind::ToolResult { call_id, content }));
        }
    }

    /// Creates a new value.
    pub fn new(label: Option<String>, working_dir: Option<String>, agent: Option<Agent>) -> Self {
        let now = Utc::now();
        let id = Uuid::new_v4().to_string();
        Self {
            metadata: Metadata {
                id,
                label,
                working_dir,
                created_at: now,
                updated_at: now,
                parent: None,
                agent_name: None,
                agent: agent.unwrap_or_default(),
                policy: None,
                status: Status::Idle,
                pid: None,
                token_estimate: 0,
                token_usage: TokenUsage {
                    method: "local_tokenizer".into(),
                    ..TokenUsage::default()
                },
                termination_reason: None,
            },
            events: Vec::new(),
        }
    }

    pub fn append(&mut self, mut event: Event) {
        let connection = &self.metadata.agent.connection;
        event.connection = (!connection.is_empty()).then(|| connection.clone());
        event.token_usage = self.metadata.token_usage.clone();
        self.events.push(event);
        let now = Utc::now();
        self.metadata.updated_at = now;
    }

    /// Performs the message operation.
    pub fn message(role: &str, content: String) -> Event {
        let role = role.into();
        Event::new(EventKind::Message { role, content })
    }
}
