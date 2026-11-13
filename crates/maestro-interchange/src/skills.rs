//! Instruction metadata supplied to a request.
use crate::source_info::SourceInfo;
use serde::{Deserialize, Serialize};

/// A described instruction file with its provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    /// Declared name, or the containing directory name.
    pub name: String,
    /// Authored description.
    pub description: String,
    /// Instruction file location.
    pub file_path: String,
    /// Directory used for relative instruction paths.
    pub base_dir: String,
    /// Origin of this file.
    pub source_info: SourceInfo,
    /// Whether this skill is omitted from model-facing prompts.
    pub disable_model_invocation: bool,
}
