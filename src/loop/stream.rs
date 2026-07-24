use crate::log::DiagLogger;
use crate::provider::{ProviderError, Response, StreamEvent};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Manages `stream.state` inside a session directory.
///
/// Format: `<unix_timestamp_secs> <chunk_count>\n`
/// - Created when streaming begins, deleted on completion or drop.
/// - External tools can poll mtime or the counter for liveness.
pub struct StreamState {
    path: PathBuf,
    chunk_count: u64,
}

impl StreamState {
    pub fn create(session_dir: &Path) -> Self {
        let prior = Self::read_chunk_count(session_dir);
        let path = session_dir.join("stream.state");
        let mut state = StreamState {
            path,
            chunk_count: prior,
        };
        state.tick();
        state
    }

    fn read_chunk_count(session_dir: &Path) -> u64 {
        let path = session_dir.join("stream.state");
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| s.split_whitespace().nth(1).and_then(|n| n.parse().ok()))
            .unwrap_or(0)
    }

    pub fn tick(&mut self) {
        self.chunk_count += 1;
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if let Ok(mut f) = fs::File::create(&self.path) {
            let _ = writeln!(f, "{} {}", ts, self.chunk_count);
        }
    }
}

pub(crate) enum StreamOutcome {
    ContinueWithTools(Response),
    Complete(Response),
    Empty,
}

pub(crate) fn reduce_response_stream(
    log: &DiagLogger,
    session_dir: &Path,
    events: Box<dyn Iterator<Item = Result<StreamEvent, ProviderError>>>,
) -> Result<StreamOutcome, String> {
    let mut state = StreamState::create(session_dir);
    for event in events {
        match event {
            Err(error) => {
                log.error("stream_error", &error.to_string());
                return Err(format!("provider error: {}", error));
            }
            Ok(StreamEvent::TextDelta(_))
            | Ok(StreamEvent::ToolCallDelta { .. })
            | Ok(StreamEvent::ReasoningDelta(_)) => state.tick(),
            Ok(StreamEvent::Complete(response)) => {
                return Ok(if response.tool_calls.is_some() {
                    StreamOutcome::ContinueWithTools(response)
                } else if response.message.is_some() {
                    StreamOutcome::Complete(response)
                } else {
                    StreamOutcome::Empty
                });
            }
        }
    }
    Err("stream ended without a Complete event".to_string())
}
