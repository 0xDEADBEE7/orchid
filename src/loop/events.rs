use crate::log::LogWriter;
use crate::session::get_session_jsonl_path;
use crate::types::{
    MessageEvent, ReasoningEvent, SessionEvent, ToolCall, ToolCallEvent, ToolResult,
    ToolResultEvent,
};

pub fn append_message(session_id: &str, content: &str) -> Result<String, String> {
    let path = get_session_jsonl_path(session_id)?;
    let event = SessionEvent::Message(MessageEvent::new("assistant", content));
    LogWriter::append(&path, &event)
}

pub fn append_system(session_id: &str, content: &str) -> Result<String, String> {
    let path = get_session_jsonl_path(session_id)?;
    let event = SessionEvent::Message(MessageEvent::new("system", content));
    LogWriter::append(&path, &event)
}

pub fn append_tool_call(session_id: &str, calls: &[ToolCall]) -> Result<String, String> {
    let path = get_session_jsonl_path(session_id)?;
    let event = SessionEvent::ToolCall(ToolCallEvent::new(calls.to_vec()));
    LogWriter::append(&path, &event)
}

pub fn append_tool_result(session_id: &str, tool_result: &ToolResult) -> Result<String, String> {
    let path = get_session_jsonl_path(session_id)?;
    let event = SessionEvent::ToolResult(ToolResultEvent::new(tool_result.clone()));
    LogWriter::append(&path, &event)
}

pub fn append_reasoning(session_id: &str, reasoning: &str) -> Result<String, String> {
    let path = get_session_jsonl_path(session_id)?;
    let event = SessionEvent::Reasoning(ReasoningEvent::new(reasoning.to_string()));
    LogWriter::append(&path, &event)
}
