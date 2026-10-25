//! Owned current-format topology and separate screen-history records.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
/// Current snapshot format version.
pub const SNAPSHOT_VERSION: u32 = 1;

/// Serializable snapshot of the workspace session.
#[derive(Serialize, Deserialize)]
pub struct SessionSnapshot {
    #[serde(default)]
    /// Stored format version.
    pub version: u32,
    /// Workspaces in display order.
    pub workspaces: Vec<WorkspaceSnapshot>,
    /// Active workspace index.
    pub active: Option<usize>,
    /// Selected sidebar index.
    pub selected: usize,
    #[serde(default)]
    /// Optional sidebar cell width.
    pub sidebar_width: Option<u16>,
    #[serde(default)]
    /// Optional sidebar section fraction.
    pub sidebar_section_split: Option<f32>,
    #[serde(default)]
    /// Collapsed workspace group identities.
    pub collapsed_space_keys: std::collections::HashSet<String>,
}

#[derive(Serialize, Deserialize)]
/// Separately persisted screen history.
pub struct SessionHistorySnapshot {
    #[serde(default)]
    /// Stored format version.
    pub version: u32,
    /// Workspaces in display order.
    pub workspaces: Vec<WorkspaceHistorySnapshot>,
}

#[derive(Serialize, Deserialize)]
/// Ordered tab histories for one workspace.
pub struct WorkspaceHistorySnapshot {
    /// Tabs in display order.
    pub tabs: Vec<TabHistorySnapshot>,
}

#[derive(Serialize, Deserialize)]
/// Pane histories indexed by raw pane identity.
pub struct TabHistorySnapshot {
    /// Panes indexed by raw pane identity.
    pub panes: HashMap<u32, PaneHistorySnapshot>,
}

#[derive(Serialize, Deserialize)]
/// Persisted workspace topology and public numbering.
pub struct WorkspaceSnapshot {
    #[serde(default)]
    /// Optional stable workspace identity.
    pub id: Option<String>,
    #[serde(default)]
    /// Optional caller-supplied display name.
    pub custom_name: Option<String>,
    /// Workspace identity directory.
    pub identity_cwd: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Optional managed-worktree membership.
    pub worktree_space: Option<crate::workspace::WorktreeSpaceMembership>,
    #[serde(default)]
    /// Public pane numbers indexed by raw identity.
    pub public_pane_numbers: HashMap<u32, usize>,
    #[serde(default)]
    /// Next public pane number.
    pub next_public_pane_number: usize,
    #[serde(default)]
    /// Public tab numbers in tab order.
    pub public_tab_numbers: Vec<usize>,
    #[serde(default)]
    /// Next public tab number.
    pub next_public_tab_number: usize,
    /// Tabs in display order.
    pub tabs: Vec<TabSnapshot>,
    #[serde(default)]
    /// Active tab index.
    pub active_tab: usize,
}

#[derive(Serialize, Deserialize)]
/// Persisted tab tree and selection.
pub struct TabSnapshot {
    #[serde(default)]
    /// Optional caller-supplied display name.
    pub custom_name: Option<String>,
    /// Persisted binary split tree.
    pub layout: LayoutSnapshot,
    /// Panes indexed by raw pane identity.
    pub panes: HashMap<u32, PaneSnapshot>,
    /// Whether the selected pane is zoomed.
    pub zoomed: bool,
    #[serde(default)]
    /// Optional focused pane identity.
    pub focused: Option<u32>,
    #[serde(default)]
    /// Optional root pane identity.
    pub root_pane: Option<u32>,
}

#[derive(Serialize, Deserialize)]
/// Persisted pane launch and working-directory data.
pub struct PaneSnapshot {
    /// Pane working directory.
    pub cwd: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Optional pane display label.
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Optional agent display name.
    pub agent_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Optional managed-agent kind.
    pub managed_agent_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Optional resume provenance.
    pub agent_session: Option<PaneAgentSessionSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Optional ordered launch arguments.
    pub launch_argv: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Agent resume provenance.
pub struct PaneAgentSessionSnapshot {
    /// Resume source.
    pub source: String,
    /// Agent identity.
    pub agent: String,
    /// Typed resume reference kind.
    pub kind: maestro_protocol::shared::AgentSessionRefKind,
    /// Resume reference value.
    pub value: String,
}

#[derive(Serialize, Deserialize)]
/// Persisted terminal history.
pub struct PaneHistorySnapshot {
    /// Terminal history bytes represented as text.
    pub ansi: String,
    /// Retained history line count.
    pub lines: usize,
}

/// Serializable BSP tree.
#[derive(Serialize, Deserialize)]
pub enum LayoutSnapshot {
    /// Raw pane identity.
    Pane(u32),
    /// Ordered adjacent child regions.
    Split {
        /// Split axis.
        direction: DirectionSnapshot,
        /// First child fraction.
        ratio: f32,
        /// First child in tree order.
        first: Box<LayoutSnapshot>,
        /// Second child in tree order.
        second: Box<LayoutSnapshot>,
    },
}

#[derive(Serialize, Deserialize)]
/// Persisted split axis.
pub enum DirectionSnapshot {
    /// Left and right children.
    Horizontal,
    /// Top and bottom children.
    Vertical,
}

#[derive(Deserialize)]
/// Header decoded before workspace bodies for version classification.
struct RawSessionSnapshot {
    #[serde(default)]
    /// Stored format version.
    version: u32,
    #[serde(default)]
    /// Workspaces in display order.
    workspaces: Vec<serde_json::Value>,
    #[serde(default)]
    /// Active workspace index.
    active: Option<usize>,
    #[serde(default)]
    /// Selected sidebar index.
    selected: usize,
    #[serde(default)]
    /// Optional sidebar cell width.
    sidebar_width: Option<u16>,
    #[serde(default)]
    /// Optional sidebar section fraction.
    sidebar_section_split: Option<f32>,
    #[serde(default)]
    /// Collapsed workspace group identities.
    collapsed_space_keys: std::collections::HashSet<String>,
}

/// Decodes a current topology after classifying its raw header.
pub(crate) fn parse_snapshot(content: &str) -> Result<SessionSnapshot, String> {
    let raw: RawSessionSnapshot = serde_json::from_str(content).map_err(|e| e.to_string())?;
    check_version(raw.version, "snapshot")?;
    Ok(SessionSnapshot {
        version: raw.version,
        workspaces: raw
            .workspaces
            .into_iter()
            .map(serde_json::from_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?,
        active: raw.active,
        selected: raw.selected,
        sidebar_width: raw.sidebar_width,
        sidebar_section_split: raw.sidebar_section_split,
        collapsed_space_keys: raw.collapsed_space_keys,
    })
}
/// Decodes complete history before classifying its version.
pub(crate) fn parse_history_snapshot(content: &str) -> Result<SessionHistorySnapshot, String> {
    let snapshot: SessionHistorySnapshot =
        serde_json::from_str(content).map_err(|e| e.to_string())?;
    check_version(snapshot.version, "history snapshot")?;
    Ok(snapshot)
}
/// Requires the current format without legacy conversion.
fn check_version(version: u32, label: &str) -> Result<(), String> {
    if version > SNAPSHOT_VERSION {
        return Err(format!(
            "{label} version {version} is newer than supported {SNAPSHOT_VERSION}"
        ));
    }
    if version < SNAPSHOT_VERSION {
        return Err(format!(
            "{label} version {version} is older than supported {SNAPSHOT_VERSION}"
        ));
    }
    Ok(())
}
/// Reads the version using the same raw-header decoding boundary.
pub(crate) fn snapshot_file_version(content: &str) -> Option<u32> {
    serde_json::from_str::<RawSessionSnapshot>(content)
        .ok()
        .map(|raw| raw.version)
}
/// Projects a live split tree into its persisted axis spelling.
#[must_use]
pub fn capture_node(node: &crate::layout::Node) -> LayoutSnapshot {
    use crate::layout::Node;
    use maestro_protocol::schema::SplitDirection;
    match node {
        Node::Pane(id) => LayoutSnapshot::Pane(id.raw()),
        Node::Split {
            direction,
            ratio,
            first,
            second,
        } => LayoutSnapshot::Split {
            direction: match direction {
                SplitDirection::Right => DirectionSnapshot::Horizontal,
                SplitDirection::Down => DirectionSnapshot::Vertical,
            },
            ratio: *ratio,
            first: Box::new(capture_node(first)),
            second: Box::new(capture_node(second)),
        },
    }
}

#[cfg(test)]
mod tests;
