use serde::{Deserialize, Serialize};

use super::hooks::HookConfiguration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RootConfig {
    pub policy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hooks: Option<HookConfiguration>,
}
