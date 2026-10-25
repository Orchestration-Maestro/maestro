//! Zoom changes for the active topology tab.

use crate::app::state::DaemonState;

impl<C: FnMut() -> u128> DaemonState<C> {
    /// Toggles zoom for a split active tab, leaving single-pane tabs unchanged.
    pub fn toggle_zoom(&mut self) {
        let Some(tab) = self
            .active
            .and_then(|index| self.topology.workspaces.get_mut(index))
            .and_then(maestro_workspace::workspace::Workspace::active_tab_mut)
        else {
            return;
        };
        if tab.layout.pane_count() > 1 {
            tab.zoomed = !tab.zoomed;
            self.session_dirty = true;
        }
    }
}
