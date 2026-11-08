//! Raw terminal title synchronization with semantic pane-update publication.

use super::App;
use crate::settings::navigation::AgentSidebarToken;
use maestro_workspace::tiling::PaneId;
use std::collections::HashSet;

/// Distinguishes spinner-only changes from changes to the displayed title text.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct TerminalTitleChanges {
    /// At least one raw terminal title changed.
    pub raw_changed: bool,
    /// At least one normalized terminal title changed.
    pub stripped_changed: bool,
}

impl<C: FnMut() -> u128> App<C> {
    /// Reports whether any configured sidebar row consumes the changed title form.
    #[must_use]
    pub fn terminal_title_sidebar_changed(&self, changes: &TerminalTitleChanges) -> bool {
        let config = &self.config.ui.sidebar.agents;
        std::iter::once(&config.rows)
            .chain(config.rows_by_agent.values())
            .flatten()
            .flatten()
            .any(|token| match token.parts().0 {
                AgentSidebarToken::TerminalTitle => changes.raw_changed,
                AgentSidebarToken::TerminalTitleStripped => changes.stripped_changed,
                _ => false,
            })
    }

    /// Synchronizes pending title origins without consuming unrelated render damage.
    pub async fn sync_pending_terminal_titles(&mut self) -> TerminalTitleChanges {
        let sources = self.launch.render_dirty.pending_terminal_title_sources();
        let changes = self.sync_terminal_titles(&sources).await;
        if self.terminal_title_sidebar_changed(&changes) {
            self.launch.render_dirty.request_generic();
            self.launch.render_notify.notify_one();
        }
        changes
    }

    /// Retains the latest raw title and publishes only normalized title changes.
    pub async fn sync_terminal_titles<S: std::hash::BuildHasher>(
        &mut self,
        sources: &HashSet<PaneId, S>,
    ) -> TerminalTitleChanges {
        let mut observations = Vec::with_capacity(sources.len());
        for pane_id in sources {
            let Some((workspace, pane)) = self.state.find_pane(*pane_id) else {
                continue;
            };
            let terminal = &pane.attached_terminal_id;
            if let Some(runtime) = self.terminal_runtimes.get(terminal) {
                observations.push((
                    workspace,
                    *pane_id,
                    terminal.clone(),
                    runtime.terminal_title(),
                ));
            }
        }
        let mut changes = TerminalTitleChanges::default();
        let mut publish = Vec::new();
        for (workspace, pane, id, title) in observations {
            let Some(terminal) = self.state.terminals.get_mut(&id) else {
                continue;
            };
            let change = terminal.set_terminal_title(title);
            changes.raw_changed |= change.raw_changed;
            changes.stripped_changed |= change.stripped_changed;
            if change.stripped_changed {
                publish.push((workspace, pane));
            }
        }
        for (workspace, pane) in publish {
            self.emit_pane_updated(workspace, pane).await;
        }
        changes
    }
}
