use orchid::client::anthropic::{SseStream, StreamEvent};
use std::io::Cursor;

mod support;

#[test]
fn test_whitespace_only_text_becomes_none() {
    let data = "event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"\\n\\n\"}}\n\n";
    let reader = Cursor::new(data.as_bytes().to_vec());
    let mut stream = SseStream::new(reader);

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
