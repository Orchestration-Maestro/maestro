//! Common protocol records.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
use serde::{Deserialize, Serialize};

/// Metadata token patch schema.
pub(super) fn metadata_token_patch_schema(
    _generator: &mut schemars::SchemaGenerator,
) -> schemars::Schema {
    schemars::json_schema!({
        "type": "object",
        "maxProperties": 16,
        "propertyNames": { "pattern": "^[A-Za-z0-9_-]{1,32}$" },
        "additionalProperties": { "type": ["string", "null"] }
    })
}

/// Metadata token values schema.
pub(super) fn metadata_token_values_schema(
    _generator: &mut schemars::SchemaGenerator,
) -> schemars::Schema {
    schemars::json_schema!({
        "type": "object",
        "maxProperties": 32,
        "propertyNames": { "pattern": "^[A-Za-z0-9_-]{1,32}$" },
        "additionalProperties": { "type": "string" }
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default)]
/// Empty params record.
#[schemars(description = "")]
pub struct EmptyParams {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Workspace target record.
#[schemars(description = "")]
pub struct WorkspaceTarget {
    /// Workspace id.
    #[schemars(description = "")]
    pub workspace_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane target record.
#[schemars(description = "")]
pub struct PaneTarget {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Tab target record.
#[schemars(description = "")]
pub struct TabTarget {
    /// Tab id.
    #[schemars(description = "")]
    pub tab_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Agent target record.
#[schemars(description = "")]
pub struct AgentTarget {
    /// Target.
    #[schemars(description = "")]
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Client window title set params record.
#[schemars(description = "")]
pub struct ClientWindowTitleSetParams {
    /// Title.
    #[schemars(description = "")]
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Split direction record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum SplitDirection {
    /// Right.
    #[schemars(description = "")]
    Right,
    /// Down.
    #[schemars(description = "")]
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Read source record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum ReadSource {
    /// Visible.
    #[schemars(description = "")]
    Visible,
    /// Recent.
    #[schemars(description = "")]
    Recent,
    /// Recent unwrapped.
    #[schemars(description = "")]
    RecentUnwrapped,
    /// Detection.
    #[schemars(description = "")]
    Detection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Read intent record.
pub enum ReadIntent {
    #[default]
    /// Interactive.
    Interactive,
    /// Passive.
    Passive,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default,
)]
#[serde(rename_all = "snake_case")]
/// Read format record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum ReadFormat {
    #[default]
    /// Text.
    #[schemars(description = "")]
    Text,
    /// Ansi.
    #[schemars(description = "")]
    Ansi,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Notification show params record.
#[schemars(description = "")]
pub struct NotificationShowParams {
    /// Title.
    #[schemars(description = "")]
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Body.
    #[schemars(description = "")]
    pub body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Position.
    #[schemars(description = "")]
    pub position: Option<crate::shared::ToastMaestroPosition>,
    #[serde(default, skip_serializing_if = "NotificationShowSound::is_none")]
    /// Sound.
    #[schemars(description = "")]
    pub sound: NotificationShowSound,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default,
)]
#[serde(rename_all = "snake_case")]
/// Notification show sound record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum NotificationShowSound {
    #[default]
    /// None.
    #[schemars(description = "")]
    None,
    /// Done.
    #[schemars(description = "")]
    Done,
    /// Request.
    #[schemars(description = "")]
    Request,
}

impl NotificationShowSound {
    /// Is none.
    #[must_use]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Notification show reason record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum NotificationShowReason {
    /// Shown.
    #[schemars(description = "")]
    Shown,
    /// Disabled.
    #[schemars(description = "")]
    Disabled,
    /// Rate limited.
    #[schemars(description = "")]
    RateLimited,
    /// No foreground client.
    #[schemars(description = "")]
    NoForegroundClient,
    /// Busy.
    #[schemars(description = "")]
    Busy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Client window title reason record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum ClientWindowTitleReason {
    /// Set.
    #[schemars(description = "")]
    Set,
    /// Cleared.
    #[schemars(description = "")]
    Cleared,
    /// No foreground client.
    #[schemars(description = "")]
    NoForegroundClient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Pane agent state record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum PaneAgentState {
    /// Idle.
    #[schemars(description = "")]
    Idle,
    /// Working.
    #[schemars(description = "")]
    Working,
    /// Blocked.
    #[schemars(description = "")]
    Blocked,
    /// Unknown.
    #[schemars(description = "")]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Agent status record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum AgentStatus {
    /// Idle.
    #[schemars(description = "")]
    Idle,
    /// Working.
    #[schemars(description = "")]
    Working,
    /// Blocked.
    #[schemars(description = "")]
    Blocked,
    /// Done.
    #[schemars(description = "")]
    Done,
    /// Unknown.
    #[schemars(description = "")]
    Unknown,
}

/// Default true.
#[must_use]
pub fn default_true() -> bool {
    true
}
