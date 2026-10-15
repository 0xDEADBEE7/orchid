//! Agents: a named prompt, connection, tool allowlist, token cap, and hooks.
use crate::config::{read_json, HookMap, Settings};
use serde::{Deserialize, Serialize};
use std::{fs, io};

/// An agent definition (`agents/<name>.json`). Sessions keep their own copy, so
/// editing an agent affects new sessions only.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct Agent {
    /// Prompt name, resolved as `prompts/<name>.md` or `.txt` when copied.
    pub prompt: String,
    /// Connection name (`connections/<name>.json`, or the built-in `echo`).
    #[serde(default)]
    pub connection: String,
    /// Tools the model is offered and allowed to run.
    #[serde(default)]
    pub tools: Vec<String>,
    /// Input-token cap checked before each request; negative or absent = none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i64>,
    /// Hooks that run for this agent's sessions, after the global hooks.
    #[serde(default, skip_serializing_if = "HookMap::is_empty")]
    pub hooks: HookMap,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentSummary {
    pub name: String,
    pub prompt: String,
    pub connection: String,
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Settings {
    /// Reads `agents/<name>.json` and checks that its prompt exists.
    pub fn agent_named(&self, name: &str) -> io::Result<Agent> {
        let agent: Agent = read_json(&self.root.join("agents").join(format!("{name}.json")))?;
        self.prompt_text(&agent.prompt)?;
        Ok(agent)
    }

    /// Summarises every agent definition, including invalid ones.
    pub fn agent_summaries(&self) -> io::Result<Vec<AgentSummary>> {
        let mut summaries = Vec::new();
        for entry in fs::read_dir(self.root.join("agents"))? {
            let path = entry?.path();
            if path.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|x| x.to_str())
                .ok_or_else(|| io::Error::other("invalid agent filename"))?
                .to_owned();
            let agent = self.agent_named(&name);
            let shown = agent.as_ref().cloned().unwrap_or_default();
            summaries.push(AgentSummary {
                name,
                prompt: shown.prompt,
                connection: shown.connection,
                valid: agent.is_ok(),
                error: agent.err().map(|error| error.to_string()),
            });
        }
        summaries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(summaries)
    }
}
