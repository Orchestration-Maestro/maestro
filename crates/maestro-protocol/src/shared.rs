//! Shared protocol records.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Agent session ref kind record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum AgentSessionRefKind {
    /// Id.
    #[schemars(description = "")]
    Id,
    /// Path.
    #[schemars(description = "")]
    Path,
}
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default,
)]
#[serde(rename_all = "kebab-case")]
/// Toast maestro position record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum ToastMaestroPosition {
    /// Top left.
    #[schemars(description = "")]
    TopLeft,
    /// Top right.
    #[schemars(description = "")]
    TopRight,
    /// Bottom left.
    #[schemars(description = "")]
    BottomLeft,
    #[default]
    /// Bottom right.
    #[schemars(description = "")]
    BottomRight,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Config reload status record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum ConfigReloadStatus {
    /// Applied.
    #[schemars(description = "")]
    Applied,
    /// Partial.
    #[schemars(description = "")]
    Partial,
    /// Failed.
    #[schemars(description = "")]
    Failed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// Windows console key data.
pub struct WindowsKeyRecord {
    /// Key down.
    pub key_down: bool,
    /// Repeat count.
    pub repeat_count: u16,
    /// Virtual key code.
    pub virtual_key_code: u16,
    /// Virtual scan code.
    pub virtual_scan_code: u16,
    /// Unicode.
    pub unicode: u16,
    /// Control key state.
    pub control_key_state: u32,
}

/// Popup size.
mod popup_size;
pub use popup_size::PopupSize;

/// An independent boolean flag in a protocol record.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(transparent)]
#[schemars(inline, description = "")]
pub struct Flag(pub bool);

impl From<bool> for Flag {
    fn from(value: bool) -> Self {
        Self(value)
    }
}
