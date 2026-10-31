//! Tool record vocabulary.
use serde::{Deserialize, Serialize};

/// Tool scheduling preference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolExecutionMode {
    /// Non-overlapping execution.
    Sequential,
    /// Concurrent execution.
    Parallel,
}

use maestro_request::{
    presence::Omittable,
    types::{TextContent, UserBlock},
};

/// Tool output with caller-selected optional field representations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentToolResult<D: Omittable, T: Omittable> {
    /// Ordered text and image output.
    pub content: Vec<UserBlock>,
    /// Caller-defined result data.
    #[serde(default, skip_serializing_if = "Omittable::is_omitted")]
    pub details: D,
    /// Requested termination.
    #[serde(default, skip_serializing_if = "Omittable::is_omitted")]
    pub terminate: T,
}

impl<D> AgentToolResult<Option<D>, Option<bool>> {
    /// Wrap text in a result with optional details and no termination request.
    #[must_use]
    pub fn text_result(text: String, details: Option<D>) -> Self {
        Self {
            content: vec![UserBlock::Text(TextContent::new(text))],
            details,
            terminate: None,
        }
    }
}
