//! Model and reasoning selection notifications.
use crate::{presence::Presence, types::ModelThinkingLevel as ThinkingLevel};
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
#[serde(bound(deserialize = "M: Deserialize<'de>"))]
pub struct ModelSelectEvent<M> {
    /// Selected descriptor.
    pub model: M,
    /// Previous descriptor.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub previous_model: Presence<M>,
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
