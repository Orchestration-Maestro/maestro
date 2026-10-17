//! Shared slash-command inventory records.

use crate::source_info::SourceInfo;
use serde::{Deserialize, Serialize};

/// An extensible command source label, serialized as a plain string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SlashCommandSource(pub String);

impl SlashCommandSource {
    /// Registered by an extension.
    pub const EXTENSION: &'static str = "extension";
    /// Supplied by a prompt template.
    pub const PROMPT: &'static str = "prompt";
    /// Supplied by a skill.
    pub const SKILL: &'static str = "skill";
}

/// One host-supplied command inventory item with owned provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlashCommandInfo {
    /// Literal command name.
    pub name: String,
    /// Optional host-supplied description; an empty string is retained.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Open command source category.
    pub source: SlashCommandSource,
    /// Host-supplied discovery provenance.
    pub source_info: SourceInfo,
}
