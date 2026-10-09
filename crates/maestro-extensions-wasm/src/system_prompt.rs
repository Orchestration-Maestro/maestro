//! Already-assembled system prompt inputs carried by extension events.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use crate::types::object;
use crate::{Presence, Skill};
use serde::{Deserialize, Serialize};
/// Supplied file text included in prompt context.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextFile {
    /// The path.
    pub path: String,
    /// The content.
    pub content: String,
}

/// Supplied options used to assemble a system prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildSystemPromptOptions {
    /// The custom prompt.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub custom_prompt: Presence<String>,
    /// The selected tools.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub selected_tools: Presence<Vec<String>>,
    /// The tool snippets.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub tool_snippets: Presence<indexmap::IndexMap<String, String>>,
    /// The prompt guidelines.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub prompt_guidelines: Presence<Vec<String>>,
    /// The append system prompt.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub append_system_prompt: Presence<String>,
    /// The supplied working directory.
    pub cwd: String,
    /// The context files.
    #[serde(
        default,
        skip_serializing_if = "Presence::is_missing",
        deserialize_with = "object::records"
    )]
    pub context_files: Presence<Vec<ContextFile>>,
    /// The skills.
    #[serde(
        default,
        skip_serializing_if = "Presence::is_missing",
        deserialize_with = "object::records"
    )]
    pub skills: Presence<Vec<Skill>>,
}
