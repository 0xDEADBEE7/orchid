use crate::client::openai::{OpenAiClient, OpenAiListResponse, OpenAiMessage};
use crate::client::openai::{OpenAiStream, to_openai_message, openai_tool_definitions};
use crate::provider::{Provider, ProviderError, Response, StreamEvent};
use crate::types::{TokenUsage, ToolCall};

use std::io::BufReader;

pub struct OpenAiApiClient<'a> {
    pub inner: &'a OpenAiClient,
}

impl Provider for OpenAiClient {
    fn send(&self, system: String, messages: Vec<crate::types::Message>) -> Result<Response, ProviderError> {
        let api = self.api_client();
        api.send(system, messages)
    }

    fn send_streaming(
        &self,
        system: String,
        messages: Vec<crate::types::Message>,
    ) -> Result<Box<dyn Iterator<Item = Result<StreamEvent, ProviderError>>>, ProviderError> {
        let api = self.api_client();
        api.send_streaming(system, messages)
    }
}

impl<'a> OpenAiApiClient<'a> {
    pub fn send(
        &self,
        system: String,
        messages: Vec<crate::types::Message>,
    ) -> Result<Response, ProviderError> {
        let body = self.build_request_body(system, messages, false)?;

        let mut headers: Vec<(&str, &str)> = vec![];
        if !self.inner.auth_header.is_empty() {
            headers.push(("authorization", &self.inner.auth_header));
        }
        let extra: Vec<(&str, &str)> = self
            .inner
            .extra_headers
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        headers.extend_from_slice(&extra);

        let response_text = self
            .inner
            .base_client
            .post_with_retry(&self.inner.api_url, body, &headers)?;

        self.parse_response(&response_text)
    }

    pub fn send_streaming(
        &self,
        system: String,
        messages: Vec<crate::types::Message>,
    ) -> Result<Box<dyn Iterator<Item = Result<StreamEvent, ProviderError>>>, ProviderError> {
        let body = self.build_request_body(system, messages, true)?;

        let mut headers: Vec<(&str, &str)> = vec![];
        if !self.inner.auth_header.is_empty() {
            headers.push(("authorization", &self.inner.auth_header));
        }
        let extra: Vec<(&str, &str)> = self
            .inner
            .extra_headers
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        headers.extend_from_slice(&extra);

        let masked: Vec<String> = headers
            .iter()
            .map(|(k, v)| {
                let tail = if v.len() > 4 { &v[v.len() - 4..] } else { v };
                format!("{}=...{}", k, tail)
            })
            .collect();
        self.inner.base_client.log_debug("resolved_headers", &masked.join(", "));

        let response = self
            .inner
            .base_client
            .post_streaming(&self.inner.api_url, body, &headers)?;

        Ok(Box::new(OpenAiStream::new(BufReader::new(response))))
    }

    pub fn build_request_body(
        &self,
        system: String,
        messages: Vec<crate::types::Message>,
        stream: bool,
    ) -> Result<String, ProviderError> {
        let mut openai_messages: Vec<OpenAiMessage> =
            messages.iter().map(to_openai_message).collect();

        if !system.is_empty() {
            openai_messages.insert(
                0,
                OpenAiMessage {
                    role: "system".to_string(),
                    content: Some(system),
                    tool_calls: None,
                    tool_call_id: None,
                },
            );
        }

        let mut body = serde_json::json!({
            "model": self.inner.model,
            "messages": openai_messages,
            "tools": openai_tool_definitions(),
        });
        // Merge user-provided params (max_tokens, max_completion_tokens, etc.).
        for (k, v) in &self.inner.params {
            body[k.as_str()] = v.clone();
        }
        if stream {
            body["stream"] = serde_json::json!(true);
        }
        serde_json::to_string(&body).map_err(|e| {
            ProviderError::InvalidResponse(format!("failed to serialize request: {}", e))
        })
    }

    fn parse_response(&self, response_text: &str) -> Result<Response, ProviderError> {
        let openai_response: OpenAiListResponse =
            serde_json::from_str(response_text).map_err(|e| {
                ProviderError::InvalidResponse(format!(
                    "failed to parse response: {} — raw: {}",
                    e,
                    &response_text[..response_text.len().min(500)]
                ))
            })?;

        let choice = openai_response
            .choices
            .first()
            .ok_or_else(|| ProviderError::InvalidResponse("no choices in response".to_string()))?;

        let mut message_text: Option<String> = None;
        let mut reasoning_text: Option<String> = None;
        let mut tool_calls_vec: Vec<ToolCall> = Vec::new();

        if let Some(ref text) = choice.message.content {
            if !text.is_empty() {
                message_text = Some(text.clone());
            }
        }

        if let Some(ref reasoning) = choice.message.reasoning_content {
            if !reasoning.is_empty() {
                reasoning_text = Some(reasoning.clone());
            }
        }

        if let Some(ref calls) = choice.message.tool_calls {
            for tc in calls {
                let args = serde_json::from_str(&tc.function.arguments).unwrap_or(serde_json::Value::Null);
                tool_calls_vec.push(ToolCall {
                    id: tc.id.clone(),
                    name: tc.function.name.clone(),
                    input: args,
                });
            }
        }

        let usage = openai_response.usage.map(|u| TokenUsage {
            input_tokens: u.prompt_tokens.unwrap_or(0),
            output_tokens: u.completion_tokens.unwrap_or(0),
        });

        Ok(Response {
            message: message_text,
            reasoning: reasoning_text,
            tool_calls: if tool_calls_vec.is_empty() { None } else { Some(tool_calls_vec) },
            usage,
            model: None,
        })
    }
}


