//! Already-produced shell-command output.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::Presence;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// `BashResult` data.
pub struct BashResult {
    /// Output.
    pub output: String,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Exit code.
    pub exit_code: Presence<f64>,
    /// Cancelled.
    pub cancelled: bool,
    /// Truncated.
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Full output path.
    pub full_output_path: Presence<String>,
}
