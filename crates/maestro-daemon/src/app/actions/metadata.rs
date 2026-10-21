//! Scheduled presentation and token expiry through retained terminal owners.

use super::PaneStateUpdate;
use crate::{app::state::DaemonState, terminal::state::TerminalState};
use maestro_workspace::layout::PaneId;
use std::time::Instant;

impl<C: FnMut() -> u128> DaemonState<C> {
    /// Selects the earliest presentation or token deadline.
    pub(crate) fn next_agent_metadata_expiry(&self) -> Option<Instant> {
        self.terminals
            .values()
            .filter_map(TerminalState::next_agent_metadata_expiry)
            .chain(
                self.terminals
                    .values()
                    .filter_map(|terminal| terminal.metadata_tokens.next_expiry()),
            )
            .chain(
                self.topology
                    .workspaces
                    .iter()
                    .filter_map(|workspace| workspace.metadata_tokens.next_expiry()),
            )
            .min()
    }

    /// Lists attached viewports before terminal mutations borrow retained state.
    fn metadata_panes(&self) -> Vec<(usize, PaneId)> {
        self.topology
            .workspaces
            .iter()
            .enumerate()
            .flat_map(|(index, workspace)| workspace.tabs.iter().map(move |tab| (index, tab)))
            .flat_map(|(index, tab)| {
                tab.layout
                    .pane_ids()
                    .into_iter()
                    .map(move |pane| (index, pane))
            })
            .collect()
    }

    /// Applies due presentation mutations through the existing state-change owner.
    pub(crate) fn expire_agent_metadata_at(
        &mut self,
        deadline: Instant,
        now: Instant,
    ) -> Vec<PaneStateUpdate> {
        self.metadata_panes()
            .into_iter()
            .filter_map(|(workspace, pane)| {
                let viewport = self.topology.workspaces[workspace].pane_state(pane)?;
                let previous_seen = viewport.seen;
                let change = self
                    .terminals
                    .get_mut(&viewport.attached_terminal_id)?
                    .expire_agent_metadata_at(deadline, now)?
                    .effective_state_change?;
                let seen = self.apply_pane_state_change(workspace, pane, &change, false)?;
                let mut update =
                    PaneStateUpdate::from_change(workspace, pane, previous_seen, change);
                update.seen = seen;
                Some(update)
            })
            .collect()
    }

    /// Removes due pane tokens and advances their public revision once per change.
    pub(crate) fn expire_pane_metadata_tokens(&mut self, now: Instant) -> Vec<(usize, PaneId)> {
        self.metadata_panes()
            .into_iter()
            .filter_map(|(workspace, pane)| {
                let id = self.topology.workspaces[workspace].terminal_id(pane)?;
                let terminal = self.terminals.get_mut(id)?;
                terminal.metadata_tokens.expire_at(now).then(|| {
                    terminal.revision = terminal.revision.saturating_add(1);
                    (workspace, pane)
                })
            })
            .collect()
    }

    /// Removes due workspace tokens, retaining changed workspace identities.
    pub(crate) fn expire_workspace_metadata_tokens(&mut self, now: Instant) -> Vec<usize> {
        self.topology
            .workspaces
            .iter_mut()
            .enumerate()
            .filter_map(|(index, workspace)| {
                workspace.metadata_tokens.expire_at(now).then_some(index)
            })
            .collect()
    }
}
