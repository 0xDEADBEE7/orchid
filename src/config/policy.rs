use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct Permissions {
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct PolicyLimits {
    #[serde(default)]
    pub token_warn_threshold: Option<u32>,
    #[serde(default)]
    pub token_hard_limit: Option<u32>,
    #[serde(default)]
    pub max_steps: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub connections: Vec<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub permissions: Permissions,
    #[serde(default)]
    pub limits: PolicyLimits,
    #[serde(default)]
    pub env: HashMap<String, String>,
}
