//! Actions protocol records.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::common::AgentStatus;
use super::common::SplitDirection;
use super::panes::PaneInfo;
use super::workspaces::WorkspaceWorktreeInfo;
use crate::shared::PopupSize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Action link params record.
#[schemars(description = "")]
pub struct ActionManifestLinkParams {
    /// Path.
    #[schemars(description = "")]
    pub path: String,
    #[serde(default = "super::common::default_true")]
    /// Enabled.
    #[schemars(description = "")]
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default)]
/// Action list params record.
#[schemars(description = "")]
pub struct ActionManifestListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Manifest identifier.
    #[schemars(description = "")]
    pub manifest_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Action unlink params record.
#[schemars(description = "")]
pub struct ActionManifestUnlinkParams {
    /// Manifest identifier.
    #[schemars(description = "")]
    pub manifest_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Action set enabled params record.
#[schemars(description = "")]
pub struct ActionManifestSetEnabledParams {
    /// Manifest identifier.
    #[schemars(description = "")]
    pub manifest_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Installed manifest info record.
#[schemars(description = "")]
pub struct ActionManifestInfo {
    /// Manifest identifier.
    #[schemars(description = "")]
    pub manifest_id: String,
    /// Name.
    #[schemars(description = "")]
    pub name: String,
    /// Version.
    #[schemars(description = "")]
    pub version: String,
    #[serde(default)]
    #[serde(alias = "min_herdr_version")]
    /// Minimum Maestro version. Accepts `min_herdr_version` as an input alias.
    #[schemars(
        description = "Minimum Maestro version. Accepts `min_herdr_version` as an input alias."
    )]
    pub min_maestro_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Description.
    #[schemars(description = "")]
    pub description: Option<String>,
    /// Manifest path.
    #[schemars(description = "")]
    pub manifest_path: String,
    /// Manifest root.
    #[schemars(description = "")]
    pub manifest_root: String,
    /// Enabled.
    #[schemars(description = "")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Platforms.
    #[schemars(description = "")]
    pub platforms: Option<Vec<ActionPlatform>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Build.
    #[schemars(description = "")]
    pub build: Vec<ActionManifestBuild>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Startup.
    #[schemars(description = "")]
    pub startup: Vec<ActionManifestStartup>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Actions.
    #[schemars(description = "")]
    pub actions: Vec<ActionManifestAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Events.
    #[schemars(description = "")]
    pub events: Vec<ActionManifestEventHook>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Panes.
    #[schemars(description = "")]
    pub panes: Vec<ActionManifestPane>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Link handlers.
    #[schemars(description = "")]
    pub link_handlers: Vec<ActionManifestLinkHandler>,
    #[serde(default)]
    /// Source.
    #[schemars(description = "")]
    pub source: PackageSourceInfo,
    /// Warnings collected at link time or on registry load (e.g. unknown event names,
    /// missing manifest file). Non-fatal — the entry is kept and surfaced by action.manifest.list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Package-source projection.
#[schemars(description = "")]
pub struct PackageSourceInfo {
    #[serde(default)]
    /// Kind.
    #[schemars(description = "")]
    pub kind: PackageSourceKind,
    #[serde(default)]
    /// Package settings source string.
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Subdir.
    #[schemars(description = "")]
    pub subdir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Requested ref.
    #[schemars(description = "")]
    pub requested_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Resolved commit.
    #[schemars(description = "")]
    pub resolved_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Managed path.
    #[schemars(description = "")]
    pub managed_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Installed unix ms.
    #[schemars(description = "")]
    pub installed_unix_ms: Option<u64>,
}

impl Default for PackageSourceInfo {
    /// Default.
    fn default() -> Self {
        Self {
            kind: PackageSourceKind::Local,
            source: String::new(),
            subdir: None,
            requested_ref: None,
            resolved_commit: None,
            managed_path: None,
            installed_unix_ms: None,
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default,
)]
#[serde(rename_all = "snake_case")]
/// Action source kind record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum PackageSourceKind {
    #[default]
    /// Local.
    #[schemars(description = "")]
    Local,
    /// Git checkout.
    #[schemars(description = "")]
    Git,
    /// Registry package.
    #[schemars(description = "")]
    Npm,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Action manifest build record.
#[schemars(description = "")]
pub struct ActionManifestBuild {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Platforms.
    #[schemars(description = "")]
    pub platforms: Option<Vec<ActionPlatform>>,
    /// Command.
    #[schemars(description = "")]
    pub command: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Action manifest startup record.
#[schemars(description = "")]
pub struct ActionManifestStartup {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Platforms.
    #[schemars(description = "")]
    pub platforms: Option<Vec<ActionPlatform>>,
    /// Command.
    #[schemars(description = "")]
    pub command: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Action manifest action record.
#[schemars(description = "")]
pub struct ActionManifestAction {
    /// Id.
    #[schemars(description = "")]
    pub id: String,
    /// Title.
    #[schemars(description = "")]
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Description.
    #[schemars(description = "")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Contexts.
    #[schemars(description = "")]
    pub contexts: Vec<ActionContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Platforms.
    #[schemars(description = "")]
    pub platforms: Option<Vec<ActionPlatform>>,
    /// Command.
    #[schemars(description = "")]
    pub command: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Action manifest event hook record.
#[schemars(description = "")]
pub struct ActionManifestEventHook {
    /// On.
    #[schemars(description = "")]
    pub on: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Platforms.
    #[schemars(description = "")]
    pub platforms: Option<Vec<ActionPlatform>>,
    /// Command.
    #[schemars(description = "")]
    pub command: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Action manifest pane record.
#[schemars(description = "")]
pub struct ActionManifestPane {
    /// Id.
    #[schemars(description = "")]
    pub id: String,
    /// Title.
    #[schemars(description = "")]
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Description.
    #[schemars(description = "")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Platforms.
    #[schemars(description = "")]
    pub platforms: Option<Vec<ActionPlatform>>,
    #[serde(default)]
    /// Placement.
    #[schemars(description = "")]
    pub placement: ActionPanePlacement,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Width.
    #[schemars(description = "")]
    pub width: Option<PopupSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Height.
    #[schemars(description = "")]
    pub height: Option<PopupSize>,
    /// Command.
    #[schemars(description = "")]
    pub command: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Action manifest link handler record.
#[schemars(description = "")]
pub struct ActionManifestLinkHandler {
    /// Id.
    #[schemars(description = "")]
    pub id: String,
    /// Title.
    #[schemars(description = "")]
    pub title: String,
    /// Pattern.
    #[schemars(description = "")]
    pub pattern: String,
    /// Action.
    #[schemars(description = "")]
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Platforms.
    #[schemars(description = "")]
    pub platforms: Option<Vec<ActionPlatform>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default)]
/// Action list params record.
#[schemars(description = "")]
pub struct ActionListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Manifest identifier.
    #[schemars(description = "")]
    pub manifest_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default)]
/// Action log list params record.
#[schemars(description = "")]
pub struct ActionLogListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Manifest identifier.
    #[schemars(description = "")]
    pub manifest_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Limit.
    #[schemars(description = "")]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Action invoke params record.
#[schemars(description = "")]
pub struct ActionInvokeParams {
    /// Manifest identifier.
    #[schemars(description = "")]
    pub action_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Manifest identifier.
    #[schemars(description = "")]
    pub manifest_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Context.
    #[schemars(description = "")]
    pub context: Option<ActionInvocationContext>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Action command log info record.
#[schemars(description = "")]
pub struct ActionCommandLogInfo {
    /// Log id.
    #[schemars(description = "")]
    pub log_id: String,
    /// Manifest identifier.
    #[schemars(description = "")]
    pub manifest_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Manifest identifier.
    #[schemars(description = "")]
    pub action_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Event.
    #[schemars(description = "")]
    pub event: Option<String>,
    /// Command.
    #[schemars(description = "")]
    pub command: Vec<String>,
    /// Status.
    #[schemars(description = "")]
    pub status: ActionCommandStatus,
    /// Started unix ms.
    #[schemars(description = "")]
    pub started_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Finished unix ms.
    #[schemars(description = "")]
    pub finished_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Exit code.
    #[schemars(description = "")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Stdout.
    #[schemars(description = "")]
    pub stdout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Stderr.
    #[schemars(description = "")]
    pub stderr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Error.
    #[schemars(description = "")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Action command status record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum ActionCommandStatus {
    /// Running.
    #[schemars(description = "")]
    Running,
    /// Succeeded.
    #[schemars(description = "")]
    Succeeded,
    /// Failed.
    #[schemars(description = "")]
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Action platform record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum ActionPlatform {
    /// Linux.
    #[schemars(description = "")]
    Linux,
    /// Macos.
    #[schemars(description = "")]
    Macos,
    /// Windows.
    #[schemars(description = "")]
    Windows,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Action context record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum ActionContext {
    /// Global.
    #[schemars(description = "")]
    Global,
    /// Workspace.
    #[schemars(description = "")]
    Workspace,
    /// Tab.
    #[schemars(description = "")]
    Tab,
    /// Pane.
    #[schemars(description = "")]
    Pane,
    /// Selection.
    #[schemars(description = "")]
    Selection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Action invocation context record.
#[schemars(description = "")]
pub struct ActionInvocationContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Workspace id.
    #[schemars(description = "")]
    pub workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Workspace label.
    #[schemars(description = "")]
    pub workspace_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Workspace cwd.
    #[schemars(description = "")]
    pub workspace_cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Worktree.
    #[schemars(description = "")]
    pub worktree: Option<WorkspaceWorktreeInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Tab id.
    #[schemars(description = "")]
    pub tab_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Tab label.
    #[schemars(description = "")]
    pub tab_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Focused pane id.
    #[schemars(description = "")]
    pub focused_pane_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Focused pane cwd.
    #[schemars(description = "")]
    pub focused_pane_cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Focused pane agent.
    #[schemars(description = "")]
    pub focused_pane_agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Focused pane status.
    #[schemars(description = "")]
    pub focused_pane_status: Option<AgentStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Selected text.
    #[schemars(description = "")]
    pub selected_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Invocation source.
    #[schemars(description = "")]
    pub invocation_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Correlation id.
    #[schemars(description = "")]
    pub correlation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Clicked url.
    #[schemars(description = "")]
    pub clicked_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Link handler id.
    #[schemars(description = "")]
    pub link_handler_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Action info record.
#[schemars(description = "")]
pub struct ActionInfo {
    /// Manifest identifier.
    #[schemars(description = "")]
    pub manifest_id: String,
    /// Manifest identifier.
    #[schemars(description = "")]
    pub action_id: String,
    /// Title.
    #[schemars(description = "")]
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Description.
    #[schemars(description = "")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Contexts.
    #[schemars(description = "")]
    pub contexts: Vec<ActionContext>,
    /// Command.
    #[schemars(description = "")]
    pub command: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Platforms.
    #[schemars(description = "")]
    pub platforms: Option<Vec<ActionPlatform>>,
}

impl ActionInfo {
    /// Qualified id.
    #[must_use]
    pub fn qualified_id(&self) -> String {
        format!("{}.{}", self.manifest_id, self.action_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Action pane open params record.
#[schemars(description = "")]
pub struct ActionPaneOpenParams {
    /// Manifest identifier.
    #[schemars(description = "")]
    pub manifest_id: String,
    /// Entrypoint.
    #[schemars(description = "")]
    pub entrypoint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Placement.
    #[schemars(description = "")]
    pub placement: Option<ActionPanePlacement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Width.
    #[schemars(description = "")]
    pub width: Option<PopupSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Height.
    #[schemars(description = "")]
    pub height: Option<PopupSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Workspace id.
    #[schemars(description = "")]
    pub workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Target pane id.
    #[schemars(description = "")]
    pub target_pane_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Direction.
    #[schemars(description = "")]
    pub direction: Option<SplitDirection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Cwd.
    #[schemars(description = "")]
    pub cwd: Option<String>,
    #[serde(default)]
    /// Focus.
    #[schemars(description = "")]
    pub focus: bool,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    /// Env.
    #[schemars(description = "")]
    pub env: HashMap<String, String>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default,
)]
#[serde(rename_all = "snake_case")]
/// Action pane placement record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum ActionPanePlacement {
    #[default]
    /// Overlay.
    #[schemars(description = "")]
    Overlay,
    /// Popup.
    #[schemars(description = "")]
    Popup,
    /// Split.
    #[schemars(description = "")]
    Split,
    /// Tab.
    #[schemars(description = "")]
    Tab,
    /// Zoomed.
    #[schemars(description = "")]
    Zoomed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Action pane focus params record.
#[schemars(description = "")]
pub struct ActionPaneFocusParams {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Action pane close params record.
#[schemars(description = "")]
pub struct ActionPaneCloseParams {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Action pane info record.
#[schemars(description = "")]
pub struct ActionPaneInfo {
    /// Manifest identifier.
    #[schemars(description = "")]
    pub manifest_id: String,
    /// Entrypoint.
    #[schemars(description = "")]
    pub entrypoint: String,
    /// Pane.
    #[schemars(description = "")]
    pub pane: PaneInfo,
}
