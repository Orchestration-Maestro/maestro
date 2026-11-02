//! Public pane observations over topology, retained metadata and live runtimes.

use super::{App, actions::pane_agent_status};
use crate::pane::PaneRuntime;
use maestro_protocol::{
    events::EventData,
    schema::{AgentSessionInfo, PaneInfo, PaneScrollInfo},
};
use maestro_workspace::layout::PaneId;

impl<C: FnMut() -> u128> App<C> {
    /// Reads one pane, awaiting the runtime-owned foreground directory observation.
    pub async fn pane_info(&self, workspace: usize, pane_id: PaneId) -> Option<PaneInfo> {
        let ws = self.state.topology.workspaces.get(workspace)?;
        let pane = ws.pane_state(pane_id)?;
        let terminal = self.state.terminals.get(&pane.attached_terminal_id)?;
        let tab_index = ws.find_tab_index_for_pane(pane_id)?;
        let runtime = self.terminal_runtimes.get(&pane.attached_terminal_id);
        let scroll = runtime
            .and_then(|runtime| runtime.scroll_metrics())
            .map(|metrics| PaneScrollInfo {
                offset_from_bottom: metrics.offset_from_bottom as u64,
                max_offset_from_bottom: metrics.max_offset_from_bottom as u64,
                viewport_rows: metrics.viewport_rows as u64,
            });
        let foreground_cwd = match runtime {
            Some(runtime) => runtime.foreground_cwd().await,
            None => None,
        };
        let presentation = terminal.effective_presentation();
        let agent_session =
            terminal
                .current_session_identity_for_persistence()
                .map(|(source, agent, session)| AgentSessionInfo {
                    source: source.into(),
                    agent: agent.into(),
                    kind: session.kind,
                    value: session.value.clone(),
                });
        Some(PaneInfo {
            pane_id: self.state.public_pane_id(workspace, pane_id)?,
            terminal_id: terminal.id.to_string(),
            workspace_id: ws.id.clone(),
            tab_id: self.state.public_tab_id(workspace, tab_index)?,
            focused: self.state.active == Some(workspace)
                && ws.active_tab == tab_index
                && ws.focused_pane_id() == Some(pane_id),
            cwd: crate::tab::cwd_for_pane(
                &ws.tabs[tab_index],
                pane_id,
                &self.state.terminals,
                &self.terminal_runtimes,
            )
            .map(|cwd| cwd.display().to_string()),
            foreground_cwd: foreground_cwd.map(|cwd| cwd.display().to_string()),
            label: terminal.manual_label.clone(),
            agent: terminal.effective_agent_label().map(str::to_owned),
            title: presentation.title,
            terminal_title: terminal.terminal_title.clone(),
            terminal_title_stripped: terminal.terminal_title_stripped(),
            display_agent: presentation.display_agent,
            agent_status: pane_agent_status(terminal.state, pane.seen),
            state_labels: presentation.state_labels,
            tokens: terminal.metadata_tokens.values(),
            agent_session,
            scroll,
            revision: terminal.revision,
        })
    }

    /// The live process of a pane, if it has one.
    pub(super) fn pane_runtime(&self, workspace: usize, pane: PaneId) -> Option<&PaneRuntime> {
        let terminal = self
            .state
            .topology
            .workspaces
            .get(workspace)?
            .terminal_id(pane)?;
        self.terminal_runtimes.get(terminal)
    }

    /// Publishes the complete pane record through the shared event history.
    pub(super) async fn emit_pane_updated(&self, workspace: usize, pane: PaneId) {
        if let Some(pane) = self.pane_info(workspace, pane).await {
            self.event_hub.push(EventData::PaneUpdated { pane });
        }
    }
}
