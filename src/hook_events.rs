use crate::model::Event;

pub fn id(event: &Event) -> &str {
    match event {
        Event::Message { event_id, .. }
        | Event::ToolCall { event_id, .. }
        | Event::ToolResult { event_id, .. }
        | Event::Reasoning { event_id, .. }
        | Event::Usage { event_id, .. }
        | Event::Termination { event_id, .. }
        | Event::Failure { event_id, .. } => event_id,
    }
}

pub fn kind(event: &Event) -> &'static str {
    match event {
        Event::Message { .. } => "message",
        Event::ToolCall { .. } => "tool_call",
        Event::ToolResult { .. } => "tool_result",
        Event::Reasoning { .. } => "reasoning",
        Event::Usage { .. } => "usage",
        Event::Termination { .. } => "termination",
        Event::Failure { .. } => "failure",
    }
}
