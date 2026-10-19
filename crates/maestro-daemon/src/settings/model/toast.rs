//! Workspace and clipboard notification preferences.

use serde::{Deserialize, Serialize, de};

pub use maestro_protocol::shared::ToastMaestroPosition;

/// Largest supported background notification delay in seconds.
pub const MAX_TOAST_DELAY_SECONDS: u64 = 3600;

/// Notification destination.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ToastDelivery {
    /// Disable background notifications.
    #[default]
    Off,
    /// Display a workspace notification.
    Workspace,
    /// Send a terminal notification.
    Terminal,
    /// Send a system notification.
    System,
}

/// Clipboard notification placement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToastClipboardPosition {
    /// Top-left corner.
    TopLeft,
    /// Top edge, centered.
    TopCenter,
    /// Top-right corner.
    TopRight,
    /// Bottom-left corner.
    BottomLeft,
    /// Bottom edge, centered.
    #[default]
    BottomCenter,
    /// Bottom-right corner.
    BottomRight,
}

/// Workspace notification placement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct WorkspaceToastConfig {
    /// Corner of the workspace.
    pub position: ToastMaestroPosition,
}

/// Clipboard notification preferences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct ClipboardToastConfig {
    /// Show a notification after copying.
    pub enabled: bool,
    /// Placement within the terminal.
    pub position: ToastClipboardPosition,
}

impl Default for ClipboardToastConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            position: ToastClipboardPosition::BottomCenter,
        }
    }
}

/// Background and clipboard notifications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToastConfig {
    /// Background notification destination.
    pub delivery: ToastDelivery,
    /// Delay before background notifications, in seconds.
    pub delay_seconds: u64,
    /// Workspace notification placement.
    pub workspace: WorkspaceToastConfig,
    /// Clipboard notification behavior.
    pub clipboard: ClipboardToastConfig,
}

impl Default for ToastConfig {
    fn default() -> Self {
        Self {
            delivery: ToastDelivery::Off,
            delay_seconds: 1,
            workspace: WorkspaceToastConfig::default(),
            clipboard: ClipboardToastConfig::default(),
        }
    }
}

/// Optional delay preserves null-as-absent without admitting null for required fields.
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct RawToastConfig {
    /// Optional destination override.
    delivery: Option<ToastDelivery>,
    /// Optional delay override.
    delay_seconds: Option<u64>,
    /// Workspace placement.
    workspace: WorkspaceToastConfig,
    /// Clipboard behavior.
    clipboard: ClipboardToastConfig,
}

impl<'de> Deserialize<'de> for ToastConfig {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = RawToastConfig::deserialize(deserializer)?;
        let delay_seconds = raw.delay_seconds.unwrap_or(1);
        if delay_seconds > MAX_TOAST_DELAY_SECONDS {
            return Err(de::Error::custom(format!(
                "multiplexer.toast.delaySeconds must be between 0 and {MAX_TOAST_DELAY_SECONDS}"
            )));
        }
        Ok(Self {
            delivery: raw.delivery.unwrap_or_default(),
            delay_seconds,
            workspace: raw.workspace,
            clipboard: raw.clipboard,
        })
    }
}
