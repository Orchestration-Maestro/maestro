//! Public agent observations reuse the pane projection and terminal lifecycle owner.

use super::App;
use maestro_protocol::schema::AgentInfo;
use maestro_workspace::tiling::PaneId;

impl<C: FnMut() -> u128> App<C> {
    /// Collects agents in workspace, tab and layout order.
    pub async fn collect_agent_infos(&self) -> Vec<AgentInfo> {
        let mut agents = Vec::new();
        let panes =
            self.state
                .topology
                .workspaces
                .iter()
                .enumerate()
                .flat_map(|(workspace, ws)| {
                    ws.tabs.iter().flat_map(move |tab| {
                        tab.layout
                            .pane_ids()
                            .into_iter()
                            .map(move |pane| (workspace, pane))
                    })
                });
        for (workspace, pane) in panes {
            if let Some(agent) = self.agent_info(workspace, pane).await {
                agents.push(agent);
            }
        }
        agents
    }

    /// Projects a live agent without synthesizing a second presentation record.
    pub async fn agent_info(&self, workspace: usize, pane_id: PaneId) -> Option<AgentInfo> {
        let ws = self.state.topology.workspaces.get(workspace)?;
        let terminal = self.state.terminals.get(ws.terminal_id(pane_id)?)?;
        if !terminal.is_agent_terminal() {
            return None;
        }
        let pane = self.pane_info(workspace, pane_id).await?;
        Some(AgentInfo {
            terminal_id: pane.terminal_id,
            name: terminal.agent_name.clone(),
            agent: pane.agent,
            title: pane.title,
            terminal_title: pane.terminal_title,
            terminal_title_stripped: pane.terminal_title_stripped,
            display_agent: pane.display_agent,
            agent_status: pane.agent_status,
            screen_detection_skipped: terminal.full_lifecycle_hook_authority_active(),
            state_labels: pane.state_labels,
            tokens: pane.tokens,
            agent_session: pane.agent_session,
            workspace_id: pane.workspace_id,
            tab_id: pane.tab_id,
            pane_id: pane.pane_id,
            focused: pane.focused.into(),
            launch_pending: terminal.managed_agent_launch_pending(),
            interactive_ready: terminal.managed_agent_interactive_ready(),
            state_change_seq: terminal.last_agent_state_change_seq.unwrap_or(0),
            cwd: pane.cwd,
            foreground_cwd: pane.foreground_cwd,
            revision: pane.revision,
        })
    }
}
