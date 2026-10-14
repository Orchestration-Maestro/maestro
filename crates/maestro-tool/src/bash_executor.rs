//! Direct-shell output records.
use maestro_request::presence::Omittable;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// `BashResult` data.
pub struct BashResult<E: Omittable, P: Omittable> {
    /// Output.
    pub output: String,
    #[serde(default, skip_serializing_if = "Omittable::is_omitted")]
    /// Exit code.
    pub exit_code: E,
    /// Cancelled.
    pub cancelled: bool,
    /// Truncated.
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Omittable::is_omitted")]
    /// Full output path.
    pub full_output_path: P,
}
