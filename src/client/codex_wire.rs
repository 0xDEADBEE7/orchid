use serde_json::Value;

pub(super) fn responses_content_type(role: &str) -> &'static str {
    if role == "assistant" {
        "output_text"
    } else {
        "input_text"
    }
}

pub(super) fn tool_definitions() -> Vec<Value> {
    crate::tools::tool_definitions()
        .into_iter()
        .map(|tool| {
            serde_json::json!({
                "type": "function",
                "name": tool["name"],
                "description": tool["description"],
                "parameters": tool["input_schema"]
            })
        })
        .collect()
}

pub(super) fn input_items(messages: &[crate::types::Message]) -> Vec<Value> {
    messages.iter().flat_map(|message| {
        if let Some(calls) = &message.tool_calls {
            return calls.iter().map(|call| serde_json::json!({
                "type": "function_call", "call_id": call.id, "name": call.name,
                "arguments": call.input.to_string()
            })).collect::<Vec<_>>();
        }
        if let Some(result) = &message.tool_result {
            return vec![serde_json::json!({
                "type": "function_call_output", "call_id": result.call_id,
                "output": result.content.to_string()
            })];
        }
        vec![serde_json::json!({
            "role": message.role,
            "content": [{"type": responses_content_type(&message.role), "text": message.content}]
        })]
    }).collect()
}

pub(super) fn parse_output(
    raw: &str,
    model: &str,
) -> Result<crate::provider::Response, crate::provider::ProviderError> {
    let values = if let Ok(v) = serde_json::from_str::<Value>(raw) {
        vec![v]
    } else {
        raw.lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .filter(|data| *data != "[DONE]")
            .filter_map(|data| serde_json::from_str::<Value>(data).ok())
            .collect()
    };
    let mut delta_text = String::new();
    let mut completed_text = String::new();
    let mut tool_calls = Vec::new();
    for value in values {
        let mut items = value["output"].as_array().cloned().unwrap_or_default();
        if let Some(item) = value.get("item") {
            items.push(item.clone());
        }
        for item in items {
            if item["type"] == "function_call" {
                let Some(arguments) = item["arguments"].as_str() else {
                    continue;
                };
                let Ok(input) = serde_json::from_str(arguments) else {
                    continue;
                };
                tool_calls.push(crate::types::ToolCall {
                    id: item["call_id"]
                        .as_str()
                        .or_else(|| item["id"].as_str())
                        .unwrap_or_default()
                        .to_string(),
                    name: item["name"].as_str().unwrap_or_default().to_string(),
                    input,
                });
            }
            if let Some(t) = item["content"][0]["text"].as_str() {
                completed_text.push_str(t);
            }
        }
        if value["type"]
            .as_str()
            .is_some_and(|kind| kind == "response.output_text.delta" || kind == "output_text.delta")
        {
            if let Some(t) = value["delta"].as_str() {
                delta_text.push_str(t);
            }
        }
        if value["type"].as_str().is_none()
            || value["type"].as_str().is_some_and(|kind| {
                kind == "response.completed" || kind == "response.output_text.done"
            })
        {
            if let Some(t) = value["output_text"].as_str() {
                completed_text.push_str(t);
            }
        }
    }
    let text = if !delta_text.is_empty() {
        delta_text
    } else {
        completed_text
    };
    if text.is_empty() && tool_calls.is_empty() {
        return Err(crate::provider::ProviderError::InvalidResponse(
            "Codex response contained no text output or tool call".into(),
        ));
    }
    Ok(crate::provider::Response {
        message: (!text.is_empty()).then_some(text),
        reasoning: None,
        tool_calls: (!tool_calls.is_empty()).then_some(tool_calls),
        usage: None,
        model: Some(model.to_string()),
    })
}
