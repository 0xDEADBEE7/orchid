use crate::config::Policy;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Idle,
    Running,
    HookRunning,
    Failed,
    Cancelled,
    Terminated,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentSnapshot {
    pub policy: Policy,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Metadata {
    pub id: String,
    pub label: Option<String>,
    pub working_dir: Option<String>,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub policy: Policy,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Message {
        event_id: String,
        timestamp: DateTime<Utc>,
        role: String,
        content: String,
        #[serde(default)]
        token_usage: TokenUsage,
    },
    ToolCall {
        event_id: String,
        timestamp: DateTime<Utc>,
        calls: Vec<ToolCall>,
        #[serde(default)]
        token_usage: TokenUsage,
    },
    ToolResult {
        event_id: String,
        timestamp: DateTime<Utc>,
        call_id: String,
        content: Value,
        #[serde(default)]
        token_usage: TokenUsage,
    },
    Reasoning {
        event_id: String,
        timestamp: DateTime<Utc>,
        content: String,
        #[serde(default)]
        token_usage: TokenUsage,
    },
    Usage {
        event_id: String,
        timestamp: DateTime<Utc>,
        input: u32,
        output: u32,
        #[serde(default)]
        token_usage: TokenUsage,
    },
    Termination {
        event_id: String,
        timestamp: DateTime<Utc>,
        reason: String,
        #[serde(default)]
        token_usage: TokenUsage,
    },
    Failure {
        event_id: String,
        timestamp: DateTime<Utc>,
        message: String,
        #[serde(default)]
        token_usage: TokenUsage,
    },
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Reply {
    pub message: String,
    pub model: String,
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
    fn set_token_usage(&mut self, token_usage: TokenUsage) {
        match self {
            Self::Message {
                token_usage: usage, ..
            }
            | Self::ToolCall {
                token_usage: usage, ..
            }
            | Self::ToolResult {
                token_usage: usage, ..
            }
            | Self::Reasoning {
                token_usage: usage, ..
            }
            | Self::Usage {
                token_usage: usage, ..
            }
            | Self::Termination {
                token_usage: usage, ..
            }
            | Self::Failure {
                token_usage: usage, ..
            } => *usage = token_usage,
        }
    }
}

impl Session {
    pub fn pending_tool_call_ids(&self) -> Vec<String> {
        let completed = self
            .events
            .iter()
            .filter_map(|event| match event {
                Event::ToolResult { call_id, .. } => Some(call_id.as_str()),
                _ => None,
            })
            .collect::<HashSet<_>>();
        self.events
            .iter()
            .flat_map(|event| match event {
                Event::ToolCall { calls, .. } => calls
                    .iter()
                    .filter(|call| !completed.contains(call.call_id.as_str()))
                    .map(|call| call.call_id.clone())
                    .collect::<Vec<_>>(),
                _ => Vec::new(),
            })
            .collect()
    }

    pub fn new(
        label: Option<String>,
        working_dir: Option<String>,
        agent: Option<AgentSnapshot>,
    ) -> Self {
        let now = Utc::now();
        let id = Uuid::new_v4().to_string();
        Self {
            metadata: Metadata {
                id,
                label,
                working_dir,
                created_at: now,
                updated_at: now,
                policy: agent.map(|snapshot| snapshot.policy).unwrap_or_default(),
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
        event.set_token_usage(self.metadata.token_usage.clone());
        self.events.push(event);
        let now = Utc::now();
        self.metadata.updated_at = now;
    }

    pub fn message(role: &str, content: String) -> Event {
        Event::Message {
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            role: role.into(),
            content,
            token_usage: TokenUsage::default(),
        }
    }
}
