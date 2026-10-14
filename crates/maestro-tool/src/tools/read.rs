//! Read tool observations.
use maestro_request::presence::Omittable;
use serde::{Deserialize, Serialize};

/// Read observations with caller-selected optional truncation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadToolDetails<T: Omittable> {
    /// Automatic truncation observations.
    #[serde(default, skip_serializing_if = "Omittable::is_omitted")]
    pub truncation: T,
}
