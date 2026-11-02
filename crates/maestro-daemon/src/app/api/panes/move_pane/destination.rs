//! Resolves where a moved pane goes before anything is taken from its source.

use super::{super::super::responses::api_error, Source};
use crate::app::App;
use maestro_protocol::schema::{ErrorBody, PaneMoveDestination, PaneMoveReason, SplitDirection};
use maestro_workspace::layout::PaneId;

/// Where a validated move puts the pane.
#[derive(Debug)]
pub(super) enum Destination {
    /// Beside a pane of another tab.
    ExistingTab {
        /// Stable handle of the receiving tab.
        tab_id: String,
        /// Pane the moved pane is split from.
        target: PaneId,
        /// Axis of the split.
        split: SplitDirection,
        /// Share of the receiving region the first child keeps.
        ratio: f32,
        /// Whether the receiving tab belongs to another workspace.
        cross_workspace: bool,
    },
    /// A new tab of a workspace.
    NewTab {
        /// Stable handle of the receiving workspace.
        workspace_id: String,
        /// Label of the new tab.
        label: Option<String>,
    },
    /// A new workspace.
    NewWorkspace {
        /// Label of the new workspace.
        label: Option<String>,
        /// Label of its first tab.
        tab_label: Option<String>,
    },
}

impl Destination {
    /// Whether the pane leaves the workspace it is in.
    pub(super) fn leaves_workspace(&self, source_workspace_id: &str) -> bool {
        match self {
            Self::ExistingTab {
                cross_workspace, ..
            } => *cross_workspace,
            Self::NewTab { workspace_id, .. } => workspace_id != source_workspace_id,
            Self::NewWorkspace { .. } => true,
        }
    }
}

/// A destination beside a pane of an existing tab, as requested.
struct BesidePane {
    /// Handle of the receiving tab, in any accepted spelling.
    tab_id: String,
    /// Handle of the pane to split, when the request names one.
    target_pane_id: Option<String>,
    /// Axis of the split.
    split: SplitDirection,
    /// Share of the receiving region the first child keeps, when the request names one.
    ratio: Option<f32>,
}

/// What a move request resolves to.
#[derive(Debug)]
pub(super) enum Resolution {
    /// Nothing may move; the answer names the reason and describes this tab.
    Unchanged(PaneMoveReason, (usize, usize)),
    /// The pane goes here.
    Move(Destination),
}

impl<C: FnMut() -> u128> App<C> {
    /// Resolves the requested destination against the live topology.
    pub(super) fn resolve_move_destination(
        &self,
        source: &Source,
        destination: PaneMoveDestination,
    ) -> Result<Resolution, ErrorBody> {
        let source_tab = (source.at.workspace, source.at.tab);
        if self.state.topology.workspaces[source_tab.0].tabs[source_tab.1].zoomed {
            return Ok(Resolution::Unchanged(PaneMoveReason::ZoomedTab, source_tab));
        }
        match destination {
            PaneMoveDestination::Tab {
                tab_id,
                target_pane_id,
                split,
                ratio,
            } => self.resolve_tab_destination(
                source,
                BesidePane {
                    tab_id,
                    target_pane_id,
                    split,
                    ratio,
                },
            ),
            PaneMoveDestination::NewTab {
                workspace_id,
                label,
            } => {
                let workspace_id = match workspace_id {
                    Some(id) => {
                        let workspace = self.target_workspace(&id)?;
                        self.state.topology.workspaces[workspace].id.clone()
                    }
                    None => source.workspace_id.clone(),
                };
                Ok(Resolution::Move(Destination::NewTab {
                    workspace_id,
                    label,
                }))
            }
            PaneMoveDestination::NewWorkspace { label, tab_label } => {
                Ok(Resolution::Move(Destination::NewWorkspace {
                    label,
                    tab_label,
                }))
            }
        }
    }

    /// Resolves a destination beside a pane of an existing tab.
    fn resolve_tab_destination(
        &self,
        source: &Source,
        beside: BesidePane,
    ) -> Result<Resolution, ErrorBody> {
        let tab_id = beside.tab_id.as_str();
        let not_found = || api_error("tab_not_found", format!("tab {tab_id} not found"));
        let (workspace, tab) = self.state.parse_tab_id(tab_id).ok_or_else(not_found)?;
        if (workspace, tab) == (source.at.workspace, source.at.tab) {
            return Ok(Resolution::Unchanged(
                PaneMoveReason::SameTab,
                (workspace, tab),
            ));
        }
        let receiving = &self.state.topology.workspaces[workspace].tabs[tab];
        if receiving.zoomed {
            return Ok(Resolution::Unchanged(
                PaneMoveReason::ZoomedTab,
                (workspace, tab),
            ));
        }
        let target = match beside.target_pane_id.as_deref() {
            Some(raw) => self.target_in_tab(raw, tab_id, (workspace, tab))?,
            None => receiving.layout.focused(),
        };
        let current_id = self
            .state
            .public_tab_id(workspace, tab)
            .ok_or_else(not_found)?;
        Ok(Resolution::Move(Destination::ExistingTab {
            tab_id: current_id,
            target,
            split: beside.split,
            ratio: beside.ratio.unwrap_or(0.5),
            cross_workspace: workspace != source.at.workspace,
        }))
    }

    /// Resolves the pane a moved pane is split from, which must be in the receiving tab.
    fn target_in_tab(
        &self,
        raw: &str,
        tab_id: &str,
        tab: (usize, usize),
    ) -> Result<PaneId, ErrorBody> {
        let (workspace, pane) = self.state.parse_pane_id(raw).ok_or_else(|| {
            api_error(
                "target_pane_not_found",
                format!("target pane {raw} not found"),
            )
        })?;
        if workspace != tab.0
            || self.state.topology.workspaces[workspace].find_tab_index_for_pane(pane)
                != Some(tab.1)
        {
            return Err(api_error(
                "target_pane_not_found",
                format!("target pane {raw} is not in tab {tab_id}"),
            ));
        }
        Ok(pane)
    }
}
