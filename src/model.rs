use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
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
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionState {
    #[serde(default = "default_agent")]
    pub agent: String,
    pub status: Status,
    pub pid: Option<u32>,
    pub last_message: Option<String>,
    pub updated_at: DateTime<Utc>,
    pub token_estimate: u32,
    pub termination_reason: Option<String>,
}

fn default_agent() -> String {
    "default".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Message {
        event_id: String,
        timestamp: DateTime<Utc>,
        role: String,
        content: String,
    },
    ToolCall {
        event_id: String,
        timestamp: DateTime<Utc>,
        calls: Vec<ToolCall>,
    },
    ToolResult {
        event_id: String,
        timestamp: DateTime<Utc>,
        call_id: String,
        content: Value,
    },
    Reasoning {
        event_id: String,
        timestamp: DateTime<Utc>,
        content: String,
    },
    Usage {
        event_id: String,
        timestamp: DateTime<Utc>,
        input: u32,
        output: u32,
    },
    Termination {
        event_id: String,
        timestamp: DateTime<Utc>,
        reason: String,
    },
    Failure {
        event_id: String,
        timestamp: DateTime<Utc>,
        message: String,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub metadata: Metadata,
    pub state: SessionState,
    pub events: Vec<Event>,
}

impl Session {
    pub fn new(label: Option<String>, working_dir: Option<String>, agent: Option<String>) -> Self {
        let now = Utc::now();
        let id = Uuid::new_v4().to_string();
        Self {
            metadata: Metadata {
                id,
                label,
                working_dir,
                created_at: now,
                updated_at: now,
            },
            state: SessionState {
                agent: agent.unwrap_or_else(|| "default".into()),
                status: Status::Idle,
                pid: None,
                last_message: None,
                updated_at: now,
                token_estimate: 0,
                termination_reason: None,
            },
            events: Vec::new(),
        }
    }

    pub fn append(&mut self, event: Event) {
        if let Event::Message { role, content, .. } = &event {
            if role == "assistant" {
                self.state.last_message = Some(content.clone());
            }
        }
        self.events.push(event);
        let now = Utc::now();
        self.metadata.updated_at = now;
        self.state.updated_at = now;
    }

    pub fn message(role: &str, content: String) -> Event {
        Event::Message {
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            role: role.into(),
            content,
        }
    }
}
