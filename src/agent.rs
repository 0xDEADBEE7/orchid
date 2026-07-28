use crate::config::{read_json, Policy, Settings};
use crate::model::AgentSnapshot;
use serde::{Deserialize, Serialize};
use std::{fs, io};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Agent {
    pub policy: String,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentSummary {
    pub name: String,
    pub policy: String,
    pub prompt: String,
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Settings {
    pub fn resolve_agent(&self, name: &str) -> io::Result<Settings> {
        let agent: Agent = read_json(&self.root.join("agents").join(format!("{name}.json")))?;
        let policy: Policy = read_json(
            &self
                .root
                .join("policies")
                .join(format!("{}.json", agent.policy)),
        )?;
        let prompt_path = self
            .root
            .join("prompts")
            .join(format!("{}.md", agent.prompt));
        let prompt_path = if prompt_path.exists() {
            prompt_path
        } else {
            self.root
                .join("prompts")
                .join(format!("{}.txt", agent.prompt))
        };
        if !prompt_path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("prompt not found: {}", agent.prompt),
            ));
        }
        Ok(Settings {
            root: self.root.clone(),
            policy_name: agent.policy,
            policy,
            prompt_name: agent.prompt,
            log_level: self.log_level.clone(),
        })
    }

    pub fn snapshot_agent(&self, name: &str) -> io::Result<AgentSnapshot> {
        let settings = self.resolve_agent(name)?;
        Ok(AgentSnapshot {
            policy: settings.policy,
            prompt: settings.prompt_name,
        })
    }

    pub fn settings_for_session(&self, id: &str, policy: &Policy) -> Settings {
        Settings {
            root: self.root.clone(),
            policy_name: "session".into(),
            policy: policy.clone(),
            prompt_name: format!("session:{id}"),
            log_level: self.log_level.clone(),
        }
    }
    pub fn agent_summaries(&self) -> io::Result<Vec<AgentSummary>> {
        let mut summaries = Vec::new();
        for entry in fs::read_dir(self.root.join("agents"))? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|x| x.to_str())
                .ok_or_else(|| io::Error::other("invalid agent filename"))?
                .to_owned();
            match read_json::<Agent>(&path).and_then(|_| self.resolve_agent(&name).map(|_| ())) {
                Ok(()) => {
                    let agent: Agent = read_json(&path)?;
                    summaries.push(AgentSummary {
                        name,
                        policy: agent.policy,
                        prompt: agent.prompt,
                        valid: true,
                        error: None,
                    });
                }
                Err(error) => {
                    let agent = read_json::<Agent>(&path);
                    summaries.push(AgentSummary {
                        name,
                        policy: agent.as_ref().map(|x| x.policy.clone()).unwrap_or_default(),
                        prompt: agent.as_ref().map(|x| x.prompt.clone()).unwrap_or_default(),
                        valid: false,
                        error: Some(error.to_string()),
                    });
                }
            }
        }
        summaries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(summaries)
    }
}
