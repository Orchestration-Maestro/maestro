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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Eq, PartialOrd, Ord, Hash)]
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
/// An unrecognized user-facing model thinking level.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseModelThinkingLevelError;
impl std::fmt::Display for ParseModelThinkingLevelError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("invalid model thinking level")
    }
}
impl std::error::Error for ParseModelThinkingLevelError {}
impl std::str::FromStr for ModelThinkingLevel {
    type Err = ParseModelThinkingLevelError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::deserialize(serde::de::value::StrDeserializer::<serde::de::value::Error>::new(value))
            .map_err(|_| ParseModelThinkingLevelError)
    }
}

impl std::fmt::Display for ModelThinkingLevel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Off => "off",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
        })
    }
}
