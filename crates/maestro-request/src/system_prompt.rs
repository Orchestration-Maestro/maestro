//! Already-assembled system prompt inputs carried by extension events, and the context
//! file names those inputs are discovered under.

use crate::extension_events::object;
use crate::{presence::Presence, skills::Skill};
use serde::{Deserialize, Serialize};
/// The context file names tried in each directory, in order.
pub const CONTEXT_FILE_NAMES: [&str; 4] = ["AGENTS.md", "AGENTS.MD", "CLAUDE.md", "CLAUDE.MD"];

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
    /// The context files owner.
    pub context_files: Presence<Vec<ContextFile>>,
    /// The skills.
    #[serde(
        default,
        skip_serializing_if = "Presence::is_missing",
        deserialize_with = "object::records"
    )]
    pub skills: Presence<Vec<Skill>>,
}
