//! Zoom changes for a topology tab.

use crate::app::state::DaemonState;
use maestro_protocol::schema::{PaneZoomMode, PaneZoomReason};
use maestro_workspace::layout::PaneId;

/// What one zoom request did to a tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneZoomOutcome {
    /// Whether the zoom state of the tab changed.
    pub changed: bool,
    /// Whether the pane gained focus.
    pub focus_changed: bool,
    /// Why the zoom state did not change.
    pub reason: Option<PaneZoomReason>,
    /// Whether the tab is zoomed afterwards.
    pub zoomed: bool,
}

impl<C: FnMut() -> u128> DaemonState<C> {
    /// Toggles zoom for the focused pane's tab, leaving single-pane tabs unchanged.
    pub fn toggle_zoom(&mut self) {
        let Some((workspace, pane)) = self.active.and_then(|index| {
            let pane = self.topology.workspaces.get(index)?.focused_pane_id()?;
            Some((index, pane))
        }) else {
            return;
        };
        self.apply_pane_zoom(workspace, pane, PaneZoomMode::Toggle);
    }

    /// Focuses a pane and applies a zoom mode to its tab, reporting what changed.
    ///
    /// A tab of one pane never zooms, and a mode that matches the tab's state changes nothing.
    pub fn apply_pane_zoom(
        &mut self,
        workspace: usize,
        pane: PaneId,
        mode: PaneZoomMode,
    ) -> Option<PaneZoomOutcome> {
        let tab = self
            .topology
            .workspaces
            .get(workspace)?
            .find_tab_index_for_pane(pane)?;
        let focus_changed = self.focus_pane_in_workspace(workspace, pane);
        let record = self
            .topology
            .workspaces
            .get_mut(workspace)?
            .tabs
            .get_mut(tab)?;
        let reason = if record.layout.pane_count() <= 1 {
            Some(PaneZoomReason::SinglePane)
        } else {
            match (mode, record.zoomed) {
                (PaneZoomMode::On, true) => Some(PaneZoomReason::AlreadyZoomed),
                (PaneZoomMode::Off, false) => Some(PaneZoomReason::AlreadyUnzoomed),
                _ => None,
            }
        };
        if reason.is_none() {
            record.zoomed = match mode {
                PaneZoomMode::Toggle => !record.zoomed,
                PaneZoomMode::On => true,
                PaneZoomMode::Off => false,
            };
        }
        let outcome = PaneZoomOutcome {
            changed: reason.is_none(),
            focus_changed,
            reason,
            zoomed: record.zoomed,
        };
        if outcome.changed {
            self.session_dirty = true;
        }
        Some(outcome)
    }
}
