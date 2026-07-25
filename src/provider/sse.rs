use super::{usage, StreamEvent};
use serde_json::Value;

pub fn parse(input: &str) -> Vec<StreamEvent> {
    let mut events = Vec::new();
    let mut calls = Vec::new();
    for data in input.lines().filter_map(|line| line.strip_prefix("data: ")) {
        if data == "[DONE]" {
            flush(&mut events, &mut calls);
            events.push(StreamEvent::Done);
            continue;
        }
        match serde_json::from_str::<Value>(data) {
            Ok(value) => handle(&value, &mut events, &mut calls),
            Err(error) => events.push(StreamEvent::Malformed(format!(
                "malformed provider content: {error}"
            ))),
        }
    }
    events
}

fn handle(value: &Value, events: &mut Vec<StreamEvent>, calls: &mut Vec<(String, String)>) {
    if let Some(tokens) = usage(value) {
        events.push(StreamEvent::Usage(tokens));
    }
    if collect_chat_call(value, calls) {
        return;
    }
    if let Some(call) = completed_codex_call(value) {
        events.push(call);
        return;
    }
    if let Some(text) = text_delta(value) {
        events.push(StreamEvent::Text(text));
    }
}

fn collect_chat_call(value: &Value, calls: &mut Vec<(String, String)>) -> bool {
    let Some(items) = value
        .pointer("/choices/0/delta/tool_calls")
        .and_then(Value::as_array)
    else {
        return false;
    };
    for item in items {
        let index = item["index"].as_u64().unwrap_or(0) as usize;
        while calls.len() <= index {
            calls.push((String::new(), String::new()));
        }
        append(&mut calls[index].0, item.pointer("/function/name"));
        append(&mut calls[index].1, item.pointer("/function/arguments"));
    }
    true
}

fn append(target: &mut String, value: Option<&Value>) {
    if let Some(value) = value.and_then(Value::as_str) {
        target.push_str(value);
    }
}

fn completed_codex_call(value: &Value) -> Option<StreamEvent> {
    if value["type"] != "response.output_item.done"
        || value.pointer("/item/type").and_then(Value::as_str) != Some("function_call")
    {
        return None;
    }
    let item = &value["item"];
    Some(StreamEvent::ToolCall {
        name: item["name"].as_str()?.into(),
        input: serde_json::from_str(item["arguments"].as_str()?).ok()?,
    })
}

fn text_delta(value: &Value) -> Option<String> {
    value["delta"]
        .as_str()
        .filter(|_| {
            value["type"]
                .as_str()
                .is_some_and(|kind| kind.ends_with("output_text.delta"))
        })
        .or_else(|| {
            value
                .pointer("/choices/0/delta/content")
                .and_then(Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/content_block_delta/delta/text")
                .and_then(Value::as_str)
        })
        .map(str::to_owned)
}

fn flush(events: &mut Vec<StreamEvent>, calls: &mut Vec<(String, String)>) {
    for (name, arguments) in calls.drain(..) {
        if let Ok(input) = serde_json::from_str(&arguments) {
            events.push(StreamEvent::ToolCall { name, input });
        }
    }
}
