//! Host-supplied slash command inventory.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::SourceInfo;
use serde::{Deserialize, Serialize};
/// The source category supplied with a slash command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SlashCommandSource {
    /// Registered by an extension.
    Extension,
    /// Supplied by a prompt template.
    Prompt,
    /// Supplied by a skill.
    Skill,
}
/// One supplied command inventory item.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlashCommandInfo {
    /// Literal command name.
    pub name: String,
    /// Optional host-supplied description.
    pub description: Option<String>,
    /// Command source category.
    pub source: SlashCommandSource,
    /// Host-supplied discovery provenance.
    pub source_info: SourceInfo,
}
