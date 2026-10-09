//! Reasoning levels supplied with model descriptors.
use serde::{Deserialize, Serialize};
/// Represent the accepted reasoning levels, with Off only on model selection.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingLevel {
    /// Minimal.
    Minimal,
    /// Low.
    Low,
    /// Medium.
    Medium,
    /// High.
    High,
    /// Xhigh.
    Xhigh,
}
/// Represent the accepted reasoning levels, with Off only on model selection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum ModelThinkingLevel {
    /// Off.
    Off,
    /// Minimal.
    Minimal,
    /// Low.
    Low,
    /// Medium.
    Medium,
    /// High.
    High,
    /// Xhigh.
    Xhigh,
}
impl From<ThinkingLevel> for ModelThinkingLevel {
    fn from(level: ThinkingLevel) -> Self {
        match level {
            ThinkingLevel::Minimal => Self::Minimal,
            ThinkingLevel::Low => Self::Low,
            ThinkingLevel::Medium => Self::Medium,
            ThinkingLevel::High => Self::High,
            ThinkingLevel::Xhigh => Self::Xhigh,
        }
    }
}
