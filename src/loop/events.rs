use crate::log::LogWriter;
use std::path::Path;
use crate::types::{
    MessageEvent, ReasoningEvent, SessionEvent, ToolCall, ToolCallEvent, ToolResult,
    ToolResultEvent,
};

pub fn append_message(session_id: &str, config_dir: &Path, content: &str) -> Result<String, String> {
    let path = transcript_path(session_id, config_dir)?;
    let event = SessionEvent::Message(MessageEvent::new("assistant", content));
    LogWriter::append(&path, &event)
}

pub fn append_system(session_id: &str, config_dir: &Path, content: &str) -> Result<String, String> {
    let path = transcript_path(session_id, config_dir)?;
    let event = SessionEvent::Message(MessageEvent::new("system", content));
    LogWriter::append(&path, &event)
}

pub fn append_tool_call(session_id: &str, config_dir: &Path, calls: &[ToolCall]) -> Result<String, String> {
    let path = transcript_path(session_id, config_dir)?;
    let event = SessionEvent::ToolCall(ToolCallEvent::new(calls.to_vec()));
    LogWriter::append(&path, &event)
}

pub fn append_tool_result(session_id: &str, config_dir: &Path, tool_result: &ToolResult) -> Result<String, String> {
    let path = transcript_path(session_id, config_dir)?;
    let event = SessionEvent::ToolResult(ToolResultEvent::new(tool_result.clone()));
    LogWriter::append(&path, &event)
}

pub fn append_reasoning(session_id: &str, config_dir: &Path, reasoning: &str) -> Result<String, String> {
    let path = transcript_path(session_id, config_dir)?;
    let event = SessionEvent::Reasoning(ReasoningEvent::new(reasoning.to_string()));
    LogWriter::append(&path, &event)
}

fn transcript_path(session_id: &str, config_dir: &Path) -> Result<std::path::PathBuf, String> {
    crate::session::get_session_jsonl_path_from_config(session_id, config_dir)
}
