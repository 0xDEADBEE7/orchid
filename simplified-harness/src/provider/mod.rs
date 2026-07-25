use crate::{
    config::Settings,
    model::{Event, Session, Usage},
};
use serde_json::Value;
use std::io;

mod defs;
mod http;
mod sse;
mod stream;
pub use defs::tools as tool_definitions;
pub use sse::parse as parse_sse;

pub trait Provider {
    fn reply(&self, prompt: &str, session: &Session) -> io::Result<String>;
    fn stream(&self, prompt: &str, session: &Session) -> io::Result<Vec<StreamEvent>> {
        Ok(vec![StreamEvent::Text(self.reply(prompt, session)?)])
    }
}

fn usage(value: &Value) -> Option<Usage> {
    Some(Usage {
        input: value
            .pointer("/usage/prompt_tokens")
            .or_else(|| value.pointer("/usage/input_tokens"))?
            .as_u64()? as u32,
        output: value
            .pointer("/usage/completion_tokens")
            .or_else(|| value.pointer("/usage/output_tokens"))?
            .as_u64()? as u32,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    Text(String),
    Usage(Usage),
    ToolCall { name: String, input: Value },
    Done,
}

pub struct LocalProvider;
impl Provider for LocalProvider {
    fn reply(&self, prompt: &str, _session: &Session) -> io::Result<String> {
        Ok(format!("local provider: {prompt}"))
    }
}

pub struct HttpProvider {
    pub connection: crate::config::Connection,
    pub root: std::path::PathBuf,
    pub tools: Vec<String>,
}

impl Provider for HttpProvider {
    fn reply(&self, prompt: &str, _session: &Session) -> io::Result<String> {
        http::reply(self, prompt)
    }

    fn stream(&self, prompt: &str, session: &Session) -> io::Result<Vec<StreamEvent>> {
        stream::run(self, prompt, session)
    }
}

fn codex_headers(
    mut request: reqwest::blocking::RequestBuilder,
    root: &std::path::Path,
    name: &str,
) -> io::Result<reqwest::blocking::RequestBuilder> {
    let value: Value = serde_json::from_slice(
        &std::fs::read(root.join("auth/tokens").join(format!("{name}.json")))
            .map_err(io::Error::other)?,
    )
    .map_err(io::Error::other)?;
    let token = value["access_token"]
        .as_str()
        .ok_or_else(|| io::Error::other("Codex access token missing"))?;
    request = request
        .header("Authorization", format!("Bearer {token}"))
        .header(
            "ChatGPT-Account-ID",
            value["account_id"].as_str().unwrap_or_default(),
        )
        .header("originator", "codex_cli_rs")
        .header("openai-beta", "responses=experimental")
        .header("Accept", "text/event-stream");
    Ok(request)
}

fn codex_curl(url: &str, body: &Value, root: &std::path::Path, name: &str) -> io::Result<String> {
    let value: Value = serde_json::from_slice(
        &std::fs::read(root.join("auth/tokens").join(format!("{name}.json")))
            .map_err(io::Error::other)?,
    )
    .map_err(io::Error::other)?;
    let token = value["access_token"]
        .as_str()
        .ok_or_else(|| io::Error::other("Codex access token missing"))?;
    let account = value["account_id"].as_str().unwrap_or_default();
    let output = std::process::Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--fail",
            "--request",
            "POST",
            url,
            "--header",
            &format!("Authorization: Bearer {token}"),
            "--header",
            &format!("ChatGPT-Account-ID: {account}"),
            "--header",
            "originator: codex_cli_rs",
            "--header",
            "openai-beta: responses=experimental",
            "--header",
            "Accept: text/event-stream",
            "--header",
            "Content-Type: application/json",
            "--data",
            &body.to_string(),
        ])
        .output()
        .map_err(io::Error::other)?;
    if output.status.success() {
        String::from_utf8(output.stdout).map_err(io::Error::other)
    } else {
        Err(io::Error::other(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }
}

pub fn run(
    provider: &dyn Provider,
    settings: &Settings,
    session: &mut Session,
    prompt: &str,
) -> io::Result<String> {
    let max_steps = settings.policy.max_steps();
    if max_steps == 0 {
        return Err(io::Error::other(
            "policy max_steps must be greater than zero",
        ));
    }
    let mut next = prompt.to_owned();
    for _ in 0..max_steps {
        let (answer, usage, calls) = collect(provider.stream(&next, session)?);
        if let Some(tokens) = usage {
            session.append(Event::Usage {
                input: tokens.input,
                output: tokens.output,
            });
        }
        if !answer.is_empty() {
            session.append(Event::Message {
                role: "assistant".into(),
                content: answer.clone(),
            });
            return Ok(answer);
        }
        if calls.is_empty() {
            return Err(io::Error::other("provider returned no text"));
        }
        next = execute_tools(settings, session, calls)?;
    }
    Err(io::Error::other("provider tool loop exceeded max_steps"))
}

fn collect(events: Vec<StreamEvent>) -> (String, Option<Usage>, Vec<(String, Value)>) {
    events
        .into_iter()
        .fold((String::new(), None, Vec::new()), |mut out, event| {
            match event {
                StreamEvent::Text(text) => out.0.push_str(&text),
                StreamEvent::Usage(tokens) => out.1 = Some(tokens),
                StreamEvent::ToolCall { name, input } => out.2.push((name, input)),
                StreamEvent::Done => {}
            }
            out
        })
}

fn execute_tools(
    settings: &Settings,
    session: &mut Session,
    calls: Vec<(String, Value)>,
) -> io::Result<String> {
    let mut results = Vec::new();
    for (name, input) in calls {
        session.append(Event::ToolCall {
            name: name.clone(),
            input: input.clone(),
        });
        let result = match name.as_str() {
            "bash" => crate::tools::bash(
                settings,
                input.get("cmd").and_then(Value::as_str).unwrap_or_default(),
            ),
            "fs_read" => crate::tools::read(
                settings,
                &input
                    .get("paths")
                    .and_then(Value::as_array)
                    .map(|paths| {
                        paths
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default(),
            ),
            "fs_edit" => crate::tools::edit(
                settings,
                input
                    .get("path")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                input.get("edits").unwrap_or(&Value::Null),
            ),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unknown provider tool",
            )),
        }?;
        results.push(result.to_string());
        session.append(Event::ToolResult { content: result });
    }
    Ok(results.join("\n"))
}

pub fn tool_allowed(settings: &Settings, name: &str) -> bool {
    settings.policy.tools().iter().any(|tool| tool == name)
}
pub fn tool_result(value: impl Into<Value>) -> Value {
    value.into()
}

pub fn command(
    store: &crate::store::Store,
    settings: &Settings,
    args: &[String],
) -> io::Result<String> {
    let id = args
        .windows(2)
        .find(|pair| pair[0] == "--id")
        .map(|pair| pair[1].clone())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "__run requires --id"))?;
    let message = args
        .iter()
        .rev()
        .find(|arg| !arg.starts_with('-') && *arg != &id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "__run requires a message"))?
        .clone();
    let mut session = store.load(&id)?;
    let local = LocalProvider;
    let configured = settings
        .policy
        .connections
        .first()
        .and_then(|name| settings.connection(name).ok())
        .map(|connection| HttpProvider {
            connection,
            root: settings.root.clone(),
            tools: settings.policy.tools().to_vec(),
        });
    let provider: &dyn Provider = configured.as_ref().map_or(&local, |provider| provider);
    match run(provider, settings, &mut session, &message) {
        Ok(reply) => {
            session.state.status = crate::model::Status::Idle;
            session.state.pid = None;
            store.save(&session)?;
            let _ = crate::hooks::run(settings, "run_end", &id);
            Ok(reply)
        }
        Err(error) => {
            session.state.status = crate::model::Status::Failed;
            session.state.pid = None;
            store.save(&session)?;
            let _ = crate::hooks::run(settings, "run_failed", &id);
            Err(error)
        }
    }
}

#[cfg(test)]
#[path = "../provider_tests.rs"]
mod tests;
