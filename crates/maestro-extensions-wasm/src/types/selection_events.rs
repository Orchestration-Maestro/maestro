//! Model and reasoning selection notifications.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{Model, Presence, ThinkingLevel};
use serde::{Deserialize, Serialize};

/// What selected the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelSelectSource {
    /// Explicit selection.
    Set,
    /// Cycling through models.
    Cycle,
    /// Restoring a saved selection.
    Restore,
}

/// A selected model and the previous selection supplied by the host.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSelectEvent {
    /// Selected descriptor.
    pub model: Model,
    /// Previous descriptor.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub previous_model: Presence<Model>,
    /// Selection source.
    pub source: ModelSelectSource,
}

/// A selected reasoning level and its previous value.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingLevelSelectEvent {
    /// Selected level.
    pub level: ThinkingLevel,
    /// Previous level.
    pub previous_level: ThinkingLevel,
}
