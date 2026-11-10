//! Temporary pane exit restores only focus that the overlay still owned.

use crate::{
    app::{App, OverlayPaneState, model::Mode},
    events::AppEvent,
};

/// Overlay ownership and tab observations taken before topology removes the pane.
pub(super) struct OverlayExit {
    /// Retained original focus and owned temporary files.
    overlay: OverlayPaneState,
    /// Whether this pane was in the active workspace and selected tab.
    was_active: bool,
    /// Whether the pane still owned focus within its tab.
    was_focused: bool,
    /// Zoom observed immediately before removal, absent for a missing tab.
    zoomed_before: Option<bool>,
}

impl<C: FnMut() -> u128> App<C> {
    /// Removes overlay bookkeeping while its tab and focus can still be observed.
    pub(super) fn take_overlay_before_exit(&mut self, event: &AppEvent) -> Option<OverlayExit> {
        let AppEvent::PaneDied { pane_id } = event else {
            return None;
        };
        let overlay = self.overlay_panes.remove(pane_id)?;
        let workspace = self.state.topology.workspaces.get(overlay.ws_idx);
        let tab = workspace.and_then(|workspace| workspace.tabs.get(overlay.tab_idx));
        Some(OverlayExit {
            was_active: self.state.active == Some(overlay.ws_idx)
                && workspace.is_some_and(|workspace| workspace.active_tab == overlay.tab_idx)
                && tab.is_some_and(|tab| tab.layout.focused() == *pane_id),
            was_focused: tab.is_some_and(|tab| tab.layout.focused() == *pane_id),
            zoomed_before: tab.map(|tab| tab.zoomed),
            overlay,
        })
    }

    /// Cleans owned files and preserves any focus or zoom chosen after the overlay opened.
    pub(super) fn restore_overlay_after_exit(&mut self, exit: OverlayExit) {
        let overlay = exit.overlay;
        for file in &overlay.temp_files {
            let _ = std::fs::remove_file(file);
        }
        let Some(workspace) = self.state.topology.workspaces.get_mut(overlay.ws_idx) else {
            return;
        };
        let Some(tab) = workspace.tabs.get_mut(overlay.tab_idx) else {
            return;
        };
        if !exit.was_focused {
            if let Some(zoomed) = exit.zoomed_before {
                tab.zoomed = zoomed;
            }
            return;
        }
        if exit.was_active {
            workspace.active_tab = overlay.tab_idx;
        }
        if tab.panes.contains_key(&overlay.previous_focus) {
            tab.layout.focus_pane(overlay.previous_focus);
        }
        tab.zoomed = overlay.previous_zoomed;
        if exit.was_active && self.state.active == Some(overlay.ws_idx) {
            self.state.mode = Mode::Terminal;
        }
    }
}
