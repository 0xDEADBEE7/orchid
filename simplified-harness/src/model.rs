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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Metadata {
    pub id: String,
    pub label: Option<String>,
    pub working_dir: Option<String>,
    pub policy: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionState {
    pub status: Status,
    pub pid: Option<u32>,
    pub last_message: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Message { role: String, content: String },
    ToolCall { name: String, input: Value },
    ToolResult { content: Value },
    Reasoning { content: String },
    Usage { input: u32, output: u32 },
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
    pub fn new(label: Option<String>, working_dir: Option<String>, policy: Option<String>) -> Self {
        let now = Utc::now();
        let id = Uuid::new_v4().to_string();
        Self {
            metadata: Metadata {
                id,
                label,
                working_dir,
                policy,
                created_at: now,
                updated_at: now,
            },
            state: SessionState {
                status: Status::Idle,
                pid: None,
                last_message: None,
                updated_at: now,
            },
            events: Vec::new(),
        }
    }

    pub fn append(&mut self, event: Event) {
        if let Event::Message { role, content } = &event {
            if role == "assistant" {
                self.state.last_message = Some(content.clone());
            }
        }
        self.events.push(event);
        let now = Utc::now();
        self.metadata.updated_at = now;
        self.state.updated_at = now;
    }
}
