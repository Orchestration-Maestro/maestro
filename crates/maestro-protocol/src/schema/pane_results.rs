//! Pane results protocol records.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
use super::panes::PaneDirection;
use super::{AgentSessionInfo, AgentStatus, ReadFormat, ReadSource, SplitDirection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane info record.
#[schemars(description = "")]
pub struct PaneInfo {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    /// Terminal id.
    #[schemars(description = "")]
    pub terminal_id: String,
    /// Workspace id.
    #[schemars(description = "")]
    pub workspace_id: String,
    /// Tab id.
    #[schemars(description = "")]
    pub tab_id: String,
    /// Focused.
    #[schemars(description = "")]
    pub focused: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Cwd.
    #[schemars(description = "")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Foreground cwd.
    #[schemars(description = "")]
    pub foreground_cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Label.
    #[schemars(description = "")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Agent.
    #[schemars(description = "")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Title.
    #[schemars(description = "")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Terminal title.
    #[schemars(description = "")]
    pub terminal_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Terminal title stripped.
    #[schemars(description = "")]
    pub terminal_title_stripped: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Display agent.
    #[schemars(description = "")]
    pub display_agent: Option<String>,
    /// Agent status.
    #[schemars(description = "")]
    pub agent_status: AgentStatus,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    /// State labels.
    #[schemars(description = "")]
    pub state_labels: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    #[schemars(schema_with = "super::common::metadata_token_values_schema")]
    /// Tokens.
    #[schemars(description = "")]
    pub tokens: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Agent session.
    #[schemars(description = "")]
    pub agent_session: Option<AgentSessionInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Scroll.
    #[schemars(description = "")]
    pub scroll: Option<PaneScrollInfo>,
    /// Revision.
    #[schemars(description = "")]
    pub revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane scroll info record.
#[schemars(description = "")]
pub struct PaneScrollInfo {
    /// Offset from bottom.
    #[schemars(description = "")]
    pub offset_from_bottom: u64,
    /// Max offset from bottom.
    #[schemars(description = "")]
    pub max_offset_from_bottom: u64,
    /// Viewport rows.
    #[schemars(description = "")]
    pub viewport_rows: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane process info record.
#[schemars(description = "")]
pub struct PaneProcessInfo {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Shell pid.
    #[schemars(description = "")]
    pub shell_pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Foreground process group id.
    #[schemars(description = "")]
    pub foreground_process_group_id: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Tty.
    #[schemars(description = "")]
    pub tty: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Foreground processes.
    #[schemars(description = "")]
    pub foreground_processes: Vec<PaneProcessInfoProcess>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane process info process record.
#[schemars(description = "")]
pub struct PaneProcessInfoProcess {
    /// Pid.
    #[schemars(description = "")]
    pub pid: u32,
    /// Name.
    #[schemars(description = "")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Argv0.
    #[schemars(description = "")]
    pub argv0: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Argv.
    #[schemars(description = "")]
    pub argv: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Cmdline.
    #[schemars(description = "")]
    pub cmdline: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Cwd.
    #[schemars(description = "")]
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane swap result record.
#[schemars(description = "")]
pub struct PaneSwapResult {
    /// Changed.
    #[schemars(description = "")]
    pub changed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Reason.
    #[schemars(description = "")]
    pub reason: Option<PaneSwapReason>,
    /// Source pane id.
    #[schemars(description = "")]
    pub source_pane_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Target pane id.
    #[schemars(description = "")]
    pub target_pane_id: Option<String>,
    /// Focused pane id.
    #[schemars(description = "")]
    pub focused_pane_id: String,
    /// Layout.
    #[schemars(description = "")]
    pub layout: PaneLayoutSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Pane swap reason record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum PaneSwapReason {
    /// No neighbor.
    #[schemars(description = "")]
    NoNeighbor,
    /// Same pane.
    #[schemars(description = "")]
    SamePane,
    /// Not found.
    #[schemars(description = "")]
    NotFound,
    /// Cross tab.
    #[schemars(description = "")]
    CrossTab,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane move result record.
#[schemars(description = "")]
pub struct PaneMoveResult {
    /// Changed.
    #[schemars(description = "")]
    pub changed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Reason.
    #[schemars(description = "")]
    pub reason: Option<PaneMoveReason>,
    /// Previous pane id.
    #[schemars(description = "")]
    pub previous_pane_id: String,
    /// Previous workspace id.
    #[schemars(description = "")]
    pub previous_workspace_id: String,
    /// Previous tab id.
    #[schemars(description = "")]
    pub previous_tab_id: String,
    /// Pane.
    #[schemars(description = "")]
    pub pane: Box<PaneInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Source layout.
    #[schemars(description = "")]
    pub source_layout: Option<Box<PaneLayoutSnapshot>>,
    /// Target layout.
    #[schemars(description = "")]
    pub target_layout: Box<PaneLayoutSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Created workspace.
    #[schemars(description = "")]
    pub created_workspace: Option<super::WorkspaceInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Created tab.
    #[schemars(description = "")]
    pub created_tab: Option<super::TabInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Closed workspace id.
    #[schemars(description = "")]
    pub closed_workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Closed tab id.
    #[schemars(description = "")]
    pub closed_tab_id: Option<String>,
    /// Focused pane id.
    #[schemars(description = "")]
    pub focused_pane_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Pane move reason record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum PaneMoveReason {
    /// Same tab.
    #[schemars(description = "")]
    SameTab,
    /// Zoomed tab.
    #[schemars(description = "")]
    ZoomedTab,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane zoom result record.
#[schemars(description = "")]
pub struct PaneZoomResult {
    /// Changed.
    #[schemars(description = "")]
    pub changed: bool,
    /// Zoom changed.
    #[schemars(description = "")]
    pub zoom_changed: bool,
    /// Focus changed.
    #[schemars(description = "")]
    pub focus_changed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Reason.
    #[schemars(description = "")]
    pub reason: Option<PaneZoomReason>,
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    /// Focused pane id.
    #[schemars(description = "")]
    pub focused_pane_id: String,
    /// Zoomed.
    #[schemars(description = "")]
    pub zoomed: crate::shared::Flag,
    /// Layout.
    #[schemars(description = "")]
    pub layout: PaneLayoutSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Pane zoom reason record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum PaneZoomReason {
    /// Single pane.
    #[schemars(description = "")]
    SinglePane,
    /// Already zoomed.
    #[schemars(description = "")]
    AlreadyZoomed,
    /// Already unzoomed.
    #[schemars(description = "")]
    AlreadyUnzoomed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane layout snapshot record.
#[schemars(description = "")]
pub struct PaneLayoutSnapshot {
    /// Workspace id.
    #[schemars(description = "")]
    pub workspace_id: String,
    /// Tab id.
    #[schemars(description = "")]
    pub tab_id: String,
    /// Zoomed.
    #[schemars(description = "")]
    pub zoomed: bool,
    /// Area.
    #[schemars(description = "")]
    pub area: PaneLayoutRect,
    /// Focused pane id.
    #[schemars(description = "")]
    pub focused_pane_id: String,
    /// Panes.
    #[schemars(description = "")]
    pub panes: Vec<PaneLayoutPane>,
    /// Splits.
    #[schemars(description = "")]
    pub splits: Vec<PaneLayoutSplit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane layout rect record.
#[schemars(description = "")]
pub struct PaneLayoutRect {
    /// X.
    #[schemars(description = "")]
    pub x: u16,
    /// Y.
    #[schemars(description = "")]
    pub y: u16,
    /// Width.
    #[schemars(description = "")]
    pub width: u16,
    /// Height.
    #[schemars(description = "")]
    pub height: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane layout pane record.
#[schemars(description = "")]
pub struct PaneLayoutPane {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    /// Focused.
    #[schemars(description = "")]
    pub focused: bool,
    /// Rect.
    #[schemars(description = "")]
    pub rect: PaneLayoutRect,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane layout split record.
#[schemars(description = "")]
pub struct PaneLayoutSplit {
    /// Id.
    #[schemars(description = "")]
    pub id: String,
    /// Direction.
    #[schemars(description = "")]
    pub direction: SplitDirection,
    /// Ratio.
    #[schemars(description = "")]
    pub ratio: f32,
    /// Rect.
    #[schemars(description = "")]
    pub rect: PaneLayoutRect,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane neighbor result record.
#[schemars(description = "")]
pub struct PaneNeighborResult {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    /// Direction.
    #[schemars(description = "")]
    pub direction: PaneDirection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Neighbor pane id.
    #[schemars(description = "")]
    pub neighbor_pane_id: Option<String>,
    /// Layout.
    #[schemars(description = "")]
    pub layout: PaneLayoutSnapshot,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane edges result record.
#[schemars(description = "")]
pub struct PaneEdgesResult {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    /// Left.
    #[schemars(description = "")]
    pub left: bool,
    /// Right.
    #[schemars(description = "")]
    pub right: bool,
    /// Up.
    #[schemars(description = "")]
    pub up: bool,
    /// Down.
    #[schemars(description = "")]
    pub down: crate::shared::Flag,
    /// Layout.
    #[schemars(description = "")]
    pub layout: PaneLayoutSnapshot,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane focus direction result record.
#[schemars(description = "")]
pub struct PaneFocusDirectionResult {
    /// Changed.
    #[schemars(description = "")]
    pub changed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Reason.
    #[schemars(description = "")]
    pub reason: Option<PaneFocusDirectionReason>,
    /// Source pane id.
    #[schemars(description = "")]
    pub source_pane_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Focused pane id.
    #[schemars(description = "")]
    pub focused_pane_id: Option<String>,
    /// Layout.
    #[schemars(description = "")]
    pub layout: PaneLayoutSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Pane focus direction reason record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum PaneFocusDirectionReason {
    /// No neighbor.
    #[schemars(description = "")]
    NoNeighbor,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane resize result record.
#[schemars(description = "")]
pub struct PaneResizeResult {
    /// Changed.
    #[schemars(description = "")]
    pub changed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Reason.
    #[schemars(description = "")]
    pub reason: Option<PaneResizeReason>,
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    /// Focused pane id.
    #[schemars(description = "")]
    pub focused_pane_id: String,
    /// Layout.
    #[schemars(description = "")]
    pub layout: PaneLayoutSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Pane resize reason record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum PaneResizeReason {
    /// Unchanged.
    #[schemars(description = "")]
    Unchanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane read result record.
#[schemars(description = "")]
pub struct PaneReadResult {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    /// Workspace id.
    #[schemars(description = "")]
    pub workspace_id: String,
    /// Tab id.
    #[schemars(description = "")]
    pub tab_id: String,
    /// Source.
    #[schemars(description = "")]
    pub source: ReadSource,
    /// Format.
    #[schemars(description = "")]
    pub format: ReadFormat,
    /// Text.
    #[schemars(description = "")]
    pub text: String,
    /// Revision.
    #[schemars(description = "")]
    pub revision: u64,
    /// Truncated.
    #[schemars(description = "")]
    pub truncated: bool,
}
