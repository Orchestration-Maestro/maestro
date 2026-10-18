//! Panes protocol records.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Pane graphics set max bytes.
pub const PANE_GRAPHICS_SET_MAX_BYTES: usize = 512 * 1024;
/// Pane graphics stream max bytes.
pub const PANE_GRAPHICS_STREAM_MAX_BYTES: usize = 16 * 1024 * 1024;
/// Pane graphics direct file max bytes.
pub const PANE_GRAPHICS_DIRECT_FILE_MAX_BYTES: usize = 400 * 1024 * 1024;
/// Pane graphics max layers per pane.
pub const PANE_GRAPHICS_MAX_LAYERS_PER_PANE: usize = 16;
/// Pane graphics max layers total.
pub const PANE_GRAPHICS_MAX_LAYERS_TOTAL: usize = 64;
/// Pane graphics max inline bytes total.
pub const PANE_GRAPHICS_MAX_INLINE_BYTES_TOTAL: usize = 64 * 1024 * 1024;
/// Pane graphics primary layer id.
pub const PANE_GRAPHICS_PRIMARY_LAYER_ID: &str = "primary";

use super::common::{PaneAgentState, ReadFormat, ReadSource, SplitDirection};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default,
)]
#[serde(rename_all = "snake_case")]
/// Pane right click target record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum PaneRightClickTarget {
    #[default]
    /// Maestro.
    #[schemars(description = "")]
    #[serde(rename = "herdr")]
    Maestro,
    /// Pane.
    #[schemars(description = "")]
    Pane,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane split params record.
#[schemars(description = "")]
pub struct PaneSplitParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Workspace id.
    #[schemars(description = "")]
    pub workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Target pane id.
    #[schemars(description = "")]
    pub target_pane_id: Option<String>,
    /// Direction.
    #[schemars(description = "")]
    pub direction: SplitDirection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Ratio.
    #[schemars(description = "")]
    pub ratio: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Cwd.
    #[schemars(description = "")]
    pub cwd: Option<String>,
    #[serde(default)]
    /// Focus.
    #[schemars(description = "")]
    pub focus: bool,
    #[serde(default)]
    /// Right click.
    #[schemars(description = "")]
    pub right_click: PaneRightClickTarget,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    /// Env.
    #[schemars(description = "")]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane input set params record.
#[schemars(description = "")]
pub struct PaneInputSetParams {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    /// Right click.
    #[schemars(description = "")]
    pub right_click: PaneRightClickTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Pane direction record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum PaneDirection {
    /// Left.
    #[schemars(description = "")]
    Left,
    /// Right.
    #[schemars(description = "")]
    Right,
    /// Up.
    #[schemars(description = "")]
    Up,
    /// Down.
    #[schemars(description = "")]
    Down,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default)]
/// Pane swap params record.
#[schemars(description = "")]
pub struct PaneSwapParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Direction.
    #[schemars(description = "")]
    pub direction: Option<PaneDirection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Source pane id.
    #[schemars(description = "")]
    pub source_pane_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Target pane id.
    #[schemars(description = "")]
    pub target_pane_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane move params record.
#[schemars(description = "")]
pub struct PaneMoveParams {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    /// Destination.
    #[schemars(description = "")]
    pub destination: PaneMoveDestination,
    #[serde(default)]
    /// Focus.
    #[schemars(description = "")]
    pub focus: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
/// Pane move destination record.
#[schemars(description = "")]
pub enum PaneMoveDestination {
    /// Tab.
    #[schemars(description = "")]
    Tab {
        /// Tab id.
        #[schemars(description = "")]
        tab_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Target pane id.
        #[schemars(description = "")]
        target_pane_id: Option<String>,
        /// Split.
        #[schemars(description = "")]
        split: SplitDirection,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Ratio.
        #[schemars(description = "")]
        ratio: Option<f32>,
    },
    /// New tab.
    #[schemars(description = "")]
    NewTab {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Workspace id.
        #[schemars(description = "")]
        workspace_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Label.
        #[schemars(description = "")]
        label: Option<String>,
    },
    /// New workspace.
    #[schemars(description = "")]
    NewWorkspace {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Label.
        #[schemars(description = "")]
        label: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Tab label.
        #[schemars(description = "")]
        tab_label: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default)]
/// Pane zoom params record.
#[schemars(description = "")]
pub struct PaneZoomParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: Option<String>,
    #[serde(default)]
    /// Mode.
    #[schemars(description = "")]
    pub mode: PaneZoomMode,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default,
)]
#[serde(rename_all = "snake_case")]
/// Pane zoom mode record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum PaneZoomMode {
    #[default]
    /// Toggle.
    #[schemars(description = "")]
    Toggle,
    /// On.
    #[schemars(description = "")]
    On,
    /// Off.
    #[schemars(description = "")]
    Off,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default)]
/// Pane layout params record.
#[schemars(description = "")]
pub struct PaneLayoutParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default)]
/// Pane process info params record.
#[schemars(description = "")]
pub struct PaneProcessInfoParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default)]
/// Layout export params record.
#[schemars(description = "")]
pub struct LayoutExportParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Tab id.
    #[schemars(description = "")]
    pub tab_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
/// Layout apply params record.
#[schemars(description = "")]
pub struct LayoutApplyParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Workspace id.
    #[schemars(description = "")]
    pub workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Tab id.
    #[schemars(description = "")]
    pub tab_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Tab label.
    #[schemars(description = "")]
    pub tab_label: Option<String>,
    #[serde(default)]
    /// Focus.
    #[schemars(description = "")]
    pub focus: bool,
    /// Root.
    #[schemars(description = "")]
    pub root: LayoutNode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
/// Layout set split ratio params record.
#[schemars(description = "")]
pub struct LayoutSetSplitRatioParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Tab id.
    #[schemars(description = "")]
    pub tab_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: Option<String>,
    /// Path.
    #[schemars(description = "")]
    pub path: Vec<bool>,
    /// Ratio.
    #[schemars(description = "")]
    pub ratio: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
/// Layout description record.
#[schemars(description = "")]
pub struct LayoutDescription {
    /// Workspace id.
    #[schemars(description = "")]
    pub workspace_id: String,
    /// Tab id.
    #[schemars(description = "")]
    pub tab_id: String,
    /// Zoomed.
    #[schemars(description = "")]
    pub zoomed: bool,
    /// Focused pane id.
    #[schemars(description = "")]
    pub focused_pane_id: String,
    /// Root.
    #[schemars(description = "")]
    pub root: LayoutNode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
/// Layout node record.
#[schemars(description = "")]
pub enum LayoutNode {
    /// Pane.
    #[schemars(description = "")]
    Pane {
        #[serde(flatten)]
        /// Pane.
        #[schemars(description = "")]
        pane: LayoutPane,
    },
    /// Split.
    #[schemars(description = "")]
    Split {
        /// Direction.
        #[schemars(description = "")]
        direction: SplitDirection,
        /// Ratio.
        #[schemars(description = "")]
        ratio: f32,
        /// First.
        #[schemars(description = "")]
        first: Box<LayoutNode>,
        /// Second.
        #[schemars(description = "")]
        second: Box<LayoutNode>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema, Default)]
/// Layout pane record.
#[schemars(description = "")]
pub struct LayoutPane {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Label.
    #[schemars(description = "")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Cwd.
    #[schemars(description = "")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Command.
    #[schemars(description = "")]
    pub command: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    /// Env.
    #[schemars(description = "")]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane neighbor params record.
#[schemars(description = "")]
pub struct PaneNeighborParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: Option<String>,
    /// Direction.
    #[schemars(description = "")]
    pub direction: PaneDirection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default)]
/// Pane edges params record.
#[schemars(description = "")]
pub struct PaneEdgesParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane focus direction params record.
#[schemars(description = "")]
pub struct PaneFocusDirectionParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: Option<String>,
    /// Direction.
    #[schemars(description = "")]
    pub direction: PaneDirection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane resize params record.
#[schemars(description = "")]
pub struct PaneResizeParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: Option<String>,
    /// Direction.
    #[schemars(description = "")]
    pub direction: PaneDirection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Amount.
    #[schemars(description = "")]
    pub amount: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default)]
/// Pane list params record.
#[schemars(description = "")]
pub struct PaneListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Workspace id.
    #[schemars(description = "")]
    pub workspace_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default)]
/// Pane current params record.
#[schemars(description = "")]
pub struct PaneCurrentParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Caller pane id.
    #[schemars(description = "")]
    pub caller_pane_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane rename params record.
#[schemars(description = "")]
pub struct PaneRenameParams {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Label.
    #[schemars(description = "")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane send text params record.
#[schemars(description = "")]
pub struct PaneSendTextParams {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    /// Text.
    #[schemars(description = "")]
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane send keys params record.
#[schemars(description = "")]
pub struct PaneSendKeysParams {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    /// Keys.
    #[schemars(description = "")]
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane send input params record.
#[schemars(description = "")]
pub struct PaneSendInputParams {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// Text.
    #[schemars(description = "")]
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Keys.
    #[schemars(description = "")]
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane read params record.
#[schemars(description = "")]
pub struct PaneReadParams {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    /// Source.
    #[schemars(description = "")]
    pub source: ReadSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Lines.
    #[schemars(description = "")]
    pub lines: Option<u32>,
    #[serde(default)]
    /// Format.
    #[schemars(description = "")]
    pub format: ReadFormat,
    #[serde(default = "super::default_true")]
    /// Strip ansi.
    #[schemars(description = "")]
    pub strip_ansi: bool,
    #[serde(skip)]
    #[schemars(skip)]
    /// Intent.
    #[schemars(description = "")]
    pub intent: super::common::ReadIntent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane report agent params record.
#[schemars(description = "")]
pub struct PaneReportAgentParams {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    /// Source.
    #[schemars(description = "")]
    pub source: String,
    /// Agent.
    #[schemars(description = "")]
    pub agent: String,
    /// State.
    #[schemars(description = "")]
    pub state: PaneAgentState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Message.
    #[schemars(description = "")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Seq.
    #[schemars(description = "")]
    pub seq: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Agent session id.
    #[schemars(description = "")]
    pub agent_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Agent session path.
    #[schemars(description = "")]
    pub agent_session_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane report agent session params record.
#[schemars(description = "")]
pub struct PaneReportAgentSessionParams {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    /// Source.
    #[schemars(description = "")]
    pub source: String,
    /// Agent.
    #[schemars(description = "")]
    pub agent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Seq.
    #[schemars(description = "")]
    pub seq: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Agent session id.
    #[schemars(description = "")]
    pub agent_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Agent session path.
    #[schemars(description = "")]
    pub agent_session_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Session start source.
    #[schemars(description = "")]
    pub session_start_source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane report metadata params record.
#[schemars(description = "")]
pub struct PaneReportMetadataParams {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    /// Source.
    #[schemars(description = "")]
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Agent.
    #[schemars(description = "")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Applies to source.
    #[schemars(description = "")]
    pub applies_to_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Title.
    #[schemars(description = "")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Display agent.
    #[schemars(description = "")]
    pub display_agent: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    /// State labels.
    #[schemars(description = "")]
    pub state_labels: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    #[schemars(schema_with = "super::common::metadata_token_patch_schema")]
    /// Tokens.
    #[schemars(description = "")]
    pub tokens: HashMap<String, Option<String>>,
    #[serde(default)]
    /// Clear title.
    #[schemars(description = "")]
    pub clear_title: bool,
    #[serde(default)]
    /// Clear display agent.
    #[schemars(description = "")]
    pub clear_display_agent: bool,
    #[serde(default)]
    /// Clear state labels.
    #[schemars(description = "")]
    pub clear_state_labels: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Seq.
    #[schemars(description = "")]
    pub seq: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1, max = 86_400_000))]
    /// Ttl ms.
    #[schemars(description = "")]
    pub ttl_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane clear agent authority params record.
#[schemars(description = "")]
pub struct PaneClearAgentAuthorityParams {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Source.
    #[schemars(description = "")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Seq.
    #[schemars(description = "")]
    pub seq: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane release agent params record.
#[schemars(description = "")]
pub struct PaneReleaseAgentParams {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    /// Source.
    #[schemars(description = "")]
    pub source: String,
    /// Agent.
    #[schemars(description = "")]
    pub agent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Seq.
    #[schemars(description = "")]
    pub seq: Option<u64>,
}

pub use super::pane_results::*;

pub use super::pane_graphics::*;
