use crate::client::sse::{SseEventMapper, SseParser};
use crate::provider::{ProviderError, StreamEvent};
use std::io::BufRead;

/// Provider-specific mapper for OpenAI SSE events.
pub struct OpenAiEventMapper {
    pub finish_reason: Option<String>,
}

impl OpenAiEventMapper {
    pub fn new() -> Self {
        OpenAiEventMapper { finish_reason: None }
    }
}

impl SseEventMapper for OpenAiEventMapper {
    fn map_event(
        &mut self,
        chunk: crate::client::sse::ParsedChunk,
        text_buf: &mut String,
        reasoning_buf: &mut String,
        tool_calls: &mut Vec<crate::client::sse::ToolCallAccumulator>,
    ) -> Option<Vec<StreamEvent>> {
        match chunk {
            crate::client::sse::ParsedChunk::Eof => None,
            crate::client::sse::ParsedChunk::Json(data) => {
                self.handle_openai_data(&data, text_buf, reasoning_buf, tool_calls)
            }
        }
    }
}

impl OpenAiEventMapper {
    fn handle_openai_data(
        &mut self,
        data: &serde_json::Value,
        text_buf: &mut String,
        reasoning_buf: &mut String,
        tool_calls: &mut Vec<crate::client::sse::ToolCallAccumulator>,
    ) -> Option<Vec<StreamEvent>> {
        // OpenAI wire format: OpenAiStreamChunk
        let choices = match data.get("choices").and_then(|v| v.as_array()) {
            Some(c) if !c.is_empty() => c,
            _ => return Some(vec![]),
        };

        let choice = &choices[0];

        // Check finish_reason for stream termination
        if let Some(ref finish_reason) = choice.get("finish_reason").and_then(|v| v.as_str()) {
            if *finish_reason == "stop" || *finish_reason == "tool_calls" {
                self.finish_reason = Some(finish_reason.to_string());
            }
        }

        let delta = match choice.get("delta").and_then(|v| v.as_object()) {
            Some(d) => d,
            None => return Some(vec![]),
        };

        // content
        if let Some(text) = delta.get("content").and_then(|v| v.as_str()) {
            if !text.is_empty() {
                text_buf.push_str(text);
                return Some(vec![StreamEvent::TextDelta(text.to_string())]);
            }
        }

        // reasoning_content (o1/o3 style)
        if let Some(reasoning) = delta.get("reasoning_content").and_then(|v| v.as_str()) {
            if !reasoning.is_empty() {
                reasoning_buf.push_str(reasoning);
                return Some(vec![StreamEvent::ReasoningDelta(reasoning.to_string())]);
            }
        }

        // tool_calls — use .get() to avoid panics on missing keys (LM Studio may omit them)
        let calls = data.get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("delta"))
            .and_then(|d| d.get("tool_calls"));

        if let Some(calls) = calls.and_then(|v| v.as_array()) {
            if calls.is_empty() {
                return Some(vec![]);
            }

            let mut events = Vec::new();
            for call in calls {
                let idx = call.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

                if let Some(ref id) = call.get("id").and_then(|v| v.as_str()) {
                    if idx >= tool_calls.len() {
                        let name = call.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()).unwrap_or("").to_string();
                        tool_calls.push(crate::client::sse::ToolCallAccumulator {
                            index: idx,
                            id: id.to_string(),
                            name: name.clone(),
                            input_json: String::new(),
                        });
                        events.push(StreamEvent::ToolCallDelta {
                            index: idx,
                            name,
                        });
                    } else if tool_calls[idx].id.is_empty() {
                        tool_calls[idx].id = id.to_string();
                    }
                }

                if let Some(ref func) = call.get("function").and_then(|f| f.as_object()) {
                    if let Some(ref args) = func.get("arguments").and_then(|v| v.as_str()) {
                        if idx < tool_calls.len() {
                            tool_calls[idx].input_json.push_str(args);
                        }
                    }
                    if let Some(ref name) = func.get("name").and_then(|v| v.as_str()) {
                        if idx < tool_calls.len() && !name.is_empty() {
                            events.push(StreamEvent::ToolCallDelta {
                                index: idx,
                                name: name.to_string(),
                            });
                        }
                    }
                }
            }
            return Some(events);
        }

        Some(vec![])
    }
}

/// Drives an OpenAI SSE stream, yielding `StreamEvent`s.
pub struct OpenAiStream<R: BufRead> {
    inner: SseParser<R, OpenAiEventMapper>,
}

impl<R: BufRead> OpenAiStream<R> {
    pub fn new(reader: R) -> Self {
        OpenAiStream {
            inner: SseParser::new(reader, OpenAiEventMapper::new()),
        }
    }
}

impl<R: BufRead> Iterator for OpenAiStream<R> {
    type Item = Result<StreamEvent, ProviderError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_whitespace_only_text_becomes_none() {
        let data = b"event: message_delta\ndata: {\"delta\":{\"content\":\"\\n\\n\"}}\n\ndata: [DONE]\n";
        let reader = Cursor::new(data.to_vec());
        let mut stream = OpenAiStream::new(reader);

        while let Some(event) = stream.next() {
            match event {
                Ok(StreamEvent::Complete(resp)) => {
                    assert!(
                        resp.message.is_none(),
                        "whitespace-only text should result in None message, got: {:?}",
                        resp.message
                    );
                }
                _ => {}
            }
        }
    }

    // --- Tests for defensive JSON access (no panics on missing keys) ---

    /// Regression test: LM Studio sends chunks without all keys.
    /// Missing `content` in delta should not panic.
    #[test]
    fn test_missing_content_delta_no_panic() {
        let data = b"event: message\ndata: {\"choices\":[{\"delta\":{}}]}\n\ndata: [DONE]\n";
        let reader = Cursor::new(data.to_vec());
        let stream = OpenAiStream::new(reader);
        // Should not panic; just yield Complete with no message.
        let events: Vec<_> = stream.collect();
        assert!(
            events.iter().any(|e| matches!(e, Ok(StreamEvent::Complete(_)))),
            "should yield Complete event"
        );
    }

    /// Regression test: missing `choices` entirely (some LM Studio responses
    /// send usage or error chunks without choices).
    #[test]
    fn test_missing_choices_no_panic() {
        let data = b"event: message\ndata: {\"id\":\"123\"}\n\ndata: [DONE]\n";
        let reader = Cursor::new(data.to_vec());
        let stream = OpenAiStream::new(reader);
        // Should not panic.
        let _events: Vec<_> = stream.collect();
    }

    /// Regression test: `delta` key missing from choice.
    #[test]
    fn test_missing_delta_no_panic() {
        let data = b"event: message\ndata: {\"choices\":[{\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n";
        let reader = Cursor::new(data.to_vec());
        let stream = OpenAiStream::new(reader);
        // Should not panic.
        let _events: Vec<_> = stream.collect();
    }

    /// Regression test: `tool_calls` key missing from delta (LM Studio omits
    /// it when the model is not calling tools).
    #[test]
    fn test_missing_tool_calls_no_panic() {
        let data = b"event: message\ndata: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\ndata: [DONE]\n";
        let reader = Cursor::new(data.to_vec());
        let stream = OpenAiStream::new(reader);
        let events: Vec<_> = stream.collect();
        assert!(
            events.iter().any(|e| matches!(e, Ok(StreamEvent::Complete(resp)) if resp.message.is_some())),
            "should yield Complete with message"
        );
    }

    /// Regression test: `finish_reason` key missing.
    #[test]
    fn test_missing_finish_reason_no_panic() {
        let data = b"event: message\ndata: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\ndata: {\"choices\":[{\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n";
        let reader = Cursor::new(data.to_vec());
        let stream = OpenAiStream::new(reader);
        // Should not panic.
        let _events: Vec<_> = stream.collect();
    }

    /// Regression test: empty choices array.
    #[test]
    fn test_empty_choices_no_panic() {
        let data = b"event: message\ndata: {\"choices\":[]}\n\ndata: [DONE]\n";
        let reader = Cursor::new(data.to_vec());
        let stream = OpenAiStream::new(reader);
        // Should not panic.
        let _events: Vec<_> = stream.collect();
    }

    /// Regression test: tool_calls present but missing `id` or `function`.
    #[test]
    fn test_tool_calls_missing_id_no_panic() {
        let data = b"event: message\ndata: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"cmd\\\":\\\"ls\\\"}\"}}]}}]}\n\ndata: [DONE]\n";
        let reader = Cursor::new(data.to_vec());
        let stream = OpenAiStream::new(reader);
        // Should not panic; should yield Complete with tool_calls (id may be empty).
        let events: Vec<_> = stream.collect();
        assert!(
            events.iter().any(|e| matches!(e, Ok(StreamEvent::Complete(_)))),
            "should yield Complete event"
        );
    }

    /// Regression test: tool_calls chunk missing `function` entirely.
    #[test]
    fn test_tool_calls_missing_function_no_panic() {
        let data = b"event: message\ndata: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\"}]}}]}\n\ndata: [DONE]\n";
        let reader = Cursor::new(data.to_vec());
        let stream = OpenAiStream::new(reader);
        // Should not panic.
        let _events: Vec<_> = stream.collect();
    }

    /// Regression test: reasoning_content present (o1/o3 style).
    #[test]
    fn test_reasoning_content_parsed() {
        let data = b"event: message\ndata: {\"choices\":[{\"delta\":{\"reasoning_content\":\"thinking...\"}}]}\n\ndata: message\ndata: {\"choices\":[{\"delta\":{\"content\":\"done\"}}]}\n\ndata: [DONE]\n";
        let reader = Cursor::new(data.to_vec());
        let stream = OpenAiStream::new(reader);
        let events: Vec<_> = stream.collect();
        assert!(
            events.iter().any(|e| matches!(e, Ok(StreamEvent::ReasoningDelta(_)))),
            "should yield ReasoningDelta event"
        );
        assert!(
            events.iter().any(|e| matches!(e, Ok(StreamEvent::TextDelta(_)))),
            "should yield TextDelta event"
        );
    }

    /// Regression test: LM Studio sends chunks with partial/missing keys
    /// interspersed — the most realistic scenario.
    #[test]
    fn test_lm_studio_like_partial_chunks_no_panic() {
        // Simulates LM Studio sending chunks that may omit content,
        // reasoning_content, tool_calls, or have them appear in any order.
        let chunk1: Vec<u8> = b"event: message\ndata: {\"choices\":[{\"delta\":{}}]}\n\n".to_vec();
        let chunk2: Vec<u8> = b"event: message\ndata: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n".to_vec();
        let chunk3: Vec<u8> = b"event: message\ndata: {\"choices\":[{\"delta\":{}}]}\n\n".to_vec();
        let chunk4: Vec<u8> = b"event: message_delta\ndata: {\"choices\":[{\"finish_reason\":\"stop\"}]}\n\n".to_vec();
        let chunk5: Vec<u8> = b"data: [DONE]\n".to_vec();
        let data: Vec<u8> = [chunk1, chunk2, chunk3, chunk4, chunk5]
            .into_iter()
            .flatten()
            .collect();
        let reader = Cursor::new(data);
        let stream = OpenAiStream::new(reader);
        // Should not panic; should yield a Complete event.
        let events: Vec<_> = stream.collect();
        assert!(
            events.iter().any(|e| matches!(e, Ok(StreamEvent::Complete(resp)) if resp.message.as_deref() == Some("Hello"))),
            "should yield Complete with expected message, got: {:?}",
            events
        );
    }

    /// Regression test: LM Studio sends tool_calls with missing `id` on
    /// the first chunk but it arrives on a later chunk.
    #[test]
    fn test_tool_calls_id_arrives_later_no_panic() {
        let chunk1: Vec<u8> = b"event: message\ndata: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"cmd\\\":\\\"ls\\\"}\"}}]}}]}\n\n".to_vec();
        let chunk2: Vec<u8> = b"event: message\ndata: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_abc123\"}]}}]}\n\n".to_vec();
        let chunk3: Vec<u8> = b"data: [DONE]\n".to_vec();
        let data: Vec<u8> = [chunk1, chunk2, chunk3]
            .into_iter()
            .flatten()
            .collect();
        let reader = Cursor::new(data);
        let stream = OpenAiStream::new(reader);
        // Should not panic.
        let events: Vec<_> = stream.collect();
        assert!(
            events.iter().any(|e| matches!(e, Ok(StreamEvent::Complete(resp)) if resp.tool_calls.is_some())),
            "should yield Complete with tool_calls"
        );
    }
}
