//! Plugins protocol records.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::common::AgentStatus;
use super::common::SplitDirection;
use super::panes::PaneInfo;
use super::workspaces::WorkspaceWorktreeInfo;
use crate::shared::PopupSize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Plugin link params record.
#[schemars(description = "")]
pub struct PluginLinkParams {
    /// Path.
    #[schemars(description = "")]
    pub path: String,
    #[serde(default = "super::common::default_true")]
    /// Enabled.
    #[schemars(description = "")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Source.
    #[schemars(description = "")]
    pub source: Option<PluginSourceInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default)]
/// Plugin list params record.
#[schemars(description = "")]
pub struct PluginListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Plugin id.
    #[schemars(description = "")]
    pub plugin_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Plugin unlink params record.
#[schemars(description = "")]
pub struct PluginUnlinkParams {
    /// Plugin id.
    #[schemars(description = "")]
    pub plugin_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Plugin set enabled params record.
#[schemars(description = "")]
pub struct PluginSetEnabledParams {
    /// Plugin id.
    #[schemars(description = "")]
    pub plugin_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Installed plugin info record.
#[schemars(description = "")]
pub struct InstalledPluginInfo {
    /// Plugin id.
    #[schemars(description = "")]
    pub plugin_id: String,
    /// Name.
    #[schemars(description = "")]
    pub name: String,
    /// Version.
    #[schemars(description = "")]
    pub version: String,
    #[serde(default)]
    #[serde(rename = "min_herdr_version")]
    /// Min maestro version.
    #[schemars(description = "")]
    pub min_maestro_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Description.
    #[schemars(description = "")]
    pub description: Option<String>,
    /// Manifest path.
    #[schemars(description = "")]
    pub manifest_path: String,
    /// Plugin root.
    #[schemars(description = "")]
    pub plugin_root: String,
    /// Enabled.
    #[schemars(description = "")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Platforms.
    #[schemars(description = "")]
    pub platforms: Option<Vec<PluginPlatform>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Build.
    #[schemars(description = "")]
    pub build: Vec<PluginManifestBuild>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Startup.
    #[schemars(description = "")]
    pub startup: Vec<PluginManifestStartup>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Actions.
    #[schemars(description = "")]
    pub actions: Vec<PluginManifestAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Events.
    #[schemars(description = "")]
    pub events: Vec<PluginManifestEventHook>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Panes.
    #[schemars(description = "")]
    pub panes: Vec<PluginManifestPane>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Link handlers.
    #[schemars(description = "")]
    pub link_handlers: Vec<PluginManifestLinkHandler>,
    #[serde(default)]
    /// Source.
    #[schemars(description = "")]
    pub source: PluginSourceInfo,
    /// Warnings collected at link time or on registry load (e.g. unknown event names,
    /// missing manifest file). Non-fatal — the entry is kept and surfaced by plugin.list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Plugin source info record.
#[schemars(description = "")]
pub struct PluginSourceInfo {
    #[serde(default)]
    /// Kind.
    #[schemars(description = "")]
    pub kind: PluginSourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Owner.
    #[schemars(description = "")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Repo.
    #[schemars(description = "")]
    pub repo: Option<String>,
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

impl Default for PluginSourceInfo {
    /// Default.
    fn default() -> Self {
        Self {
            kind: PluginSourceKind::Local,
            owner: None,
            repo: None,
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
/// Plugin source kind record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum PluginSourceKind {
    #[default]
    /// Local.
    #[schemars(description = "")]
    Local,
    /// Github.
    #[schemars(description = "")]
    Github,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Plugin manifest build record.
#[schemars(description = "")]
pub struct PluginManifestBuild {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Platforms.
    #[schemars(description = "")]
    pub platforms: Option<Vec<PluginPlatform>>,
    /// Command.
    #[schemars(description = "")]
    pub command: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Plugin manifest startup record.
#[schemars(description = "")]
pub struct PluginManifestStartup {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Platforms.
    #[schemars(description = "")]
    pub platforms: Option<Vec<PluginPlatform>>,
    /// Command.
    #[schemars(description = "")]
    pub command: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Plugin manifest action record.
#[schemars(description = "")]
pub struct PluginManifestAction {
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
    pub contexts: Vec<PluginActionContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Platforms.
    #[schemars(description = "")]
    pub platforms: Option<Vec<PluginPlatform>>,
    /// Command.
    #[schemars(description = "")]
    pub command: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Plugin manifest event hook record.
#[schemars(description = "")]
pub struct PluginManifestEventHook {
    /// On.
    #[schemars(description = "")]
    pub on: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Platforms.
    #[schemars(description = "")]
    pub platforms: Option<Vec<PluginPlatform>>,
    /// Command.
    #[schemars(description = "")]
    pub command: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Plugin manifest pane record.
#[schemars(description = "")]
pub struct PluginManifestPane {
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
    pub platforms: Option<Vec<PluginPlatform>>,
    #[serde(default)]
    /// Placement.
    #[schemars(description = "")]
    pub placement: PluginPanePlacement,
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
/// Plugin manifest link handler record.
#[schemars(description = "")]
pub struct PluginManifestLinkHandler {
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
    pub platforms: Option<Vec<PluginPlatform>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default)]
/// Plugin action list params record.
#[schemars(description = "")]
pub struct PluginActionListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Plugin id.
    #[schemars(description = "")]
    pub plugin_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default)]
/// Plugin log list params record.
#[schemars(description = "")]
pub struct PluginLogListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Plugin id.
    #[schemars(description = "")]
    pub plugin_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Limit.
    #[schemars(description = "")]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Plugin action invoke params record.
#[schemars(description = "")]
pub struct PluginActionInvokeParams {
    /// Action id.
    #[schemars(description = "")]
    pub action_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Plugin id.
    #[schemars(description = "")]
    pub plugin_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Context.
    #[schemars(description = "")]
    pub context: Option<PluginInvocationContext>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Plugin command log info record.
#[schemars(description = "")]
pub struct PluginCommandLogInfo {
    /// Log id.
    #[schemars(description = "")]
    pub log_id: String,
    /// Plugin id.
    #[schemars(description = "")]
    pub plugin_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Action id.
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
    pub status: PluginCommandStatus,
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
/// Plugin command status record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum PluginCommandStatus {
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
/// Plugin platform record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum PluginPlatform {
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
/// Plugin action context record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum PluginActionContext {
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
/// Plugin invocation context record.
#[schemars(description = "")]
pub struct PluginInvocationContext {
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
/// Plugin action info record.
#[schemars(description = "")]
pub struct PluginActionInfo {
    /// Plugin id.
    #[schemars(description = "")]
    pub plugin_id: String,
    /// Action id.
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
    pub contexts: Vec<PluginActionContext>,
    /// Command.
    #[schemars(description = "")]
    pub command: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Platforms.
    #[schemars(description = "")]
    pub platforms: Option<Vec<PluginPlatform>>,
}

impl PluginActionInfo {
    /// Qualified id.
    #[must_use]
    pub fn qualified_id(&self) -> String {
        format!("{}.{}", self.plugin_id, self.action_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Plugin pane open params record.
#[schemars(description = "")]
pub struct PluginPaneOpenParams {
    /// Plugin id.
    #[schemars(description = "")]
    pub plugin_id: String,
    /// Entrypoint.
    #[schemars(description = "")]
    pub entrypoint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Placement.
    #[schemars(description = "")]
    pub placement: Option<PluginPanePlacement>,
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
/// Plugin pane placement record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum PluginPanePlacement {
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
/// Plugin pane focus params record.
#[schemars(description = "")]
pub struct PluginPaneFocusParams {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Plugin pane close params record.
#[schemars(description = "")]
pub struct PluginPaneCloseParams {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Plugin pane info record.
#[schemars(description = "")]
pub struct PluginPaneInfo {
    /// Plugin id.
    #[schemars(description = "")]
    pub plugin_id: String,
    /// Entrypoint.
    #[schemars(description = "")]
    pub entrypoint: String,
    /// Pane.
    #[schemars(description = "")]
    pub pane: PaneInfo,
}
