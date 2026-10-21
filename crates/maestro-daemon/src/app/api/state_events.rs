//! Effective agent transition publication through the shared protocol payloads.

use crate::app::{
    App,
    actions::{AgentRelease, PaneStateUpdate, pane_agent_status},
};
use maestro_protocol::events::EventData;

impl<C: FnMut() -> u128> App<C> {
    /// Synchronizes detection controls after retained state accepts the observation.
    pub(super) fn sync_runtime_detection_controls(
        &self,
        released: Option<(
            maestro_workspace::layout::PaneId,
            maestro_detection::agents::Agent,
        )>,
        updates: &[PaneStateUpdate],
    ) {
        for pane in self
            .state
            .topology
            .workspaces
            .iter()
            .flat_map(|workspace| &workspace.tabs)
            .flat_map(|tab| tab.panes.values())
        {
            let Some(terminal) = self.state.terminals.get(&pane.attached_terminal_id) else {
                continue;
            };
            let Some(runtime) = self.terminal_runtimes.get(&pane.attached_terminal_id) else {
                continue;
            };
            runtime.set_full_lifecycle_authority_active(
                terminal.full_lifecycle_hook_authority_active(),
            );
        }
        if let Some((pane, agent)) = released
            && updates.iter().any(|update| update.pane_id == pane)
            && let Some((workspace, _)) = self.state.find_pane(pane)
            && let Some(terminal) = self.state.topology.workspaces[workspace].terminal_id(pane)
            && let Some(runtime) = self.terminal_runtimes.get(terminal)
        {
            runtime.begin_graceful_release(agent);
        }
    }

    /// Publishes completion and death before the pane loses its public identity.
    pub(super) fn publish_pane_process_exit(&mut self, event: &crate::events::AppEvent) {
        let crate::events::AppEvent::PaneDied { pane_id } = event else {
            return;
        };
        let previous_toast = self.state.toast.clone();
        if let Some(update) = self.state.publish_pane_process_exit_if_agent(*pane_id) {
            self.refresh_new_workspace_toast_context_for_update(&update, previous_toast.as_ref());
            self.emit_pane_state_update(&update);
        }
    }

    /// Announces a terminal pane exit only when no replacement process took ownership.
    pub(super) fn emit_pane_exit(&self, event: &crate::events::AppEvent) {
        let crate::events::AppEvent::PaneDied { pane_id } = event else {
            return;
        };
        if let Some((workspace, _)) = self.state.find_pane(*pane_id)
            && let Some(public_pane) = self.state.public_pane_id(workspace, *pane_id)
        {
            self.event_hub.push(EventData::PaneExited {
                pane_id: public_pane,
                workspace_id: self.state.topology.workspaces[workspace].id.clone(),
            });
        }
    }

    /// Emits identity release even when its final execution state did not change.
    pub(in crate::app) fn emit_pane_state_update(&self, update: &PaneStateUpdate) {
        let Some(pane_id) = self.state.public_pane_id(update.ws_idx, update.pane_id) else {
            return;
        };
        let Some(workspace) = self.state.topology.workspaces.get(update.ws_idx) else {
            return;
        };
        let workspace_id = workspace.id.clone();
        if update.previous_agent_label != update.agent_label
            || matches!(update.release, AgentRelease::Released(_))
        {
            let final_status = match update.release {
                AgentRelease::Retained => None,
                AgentRelease::Released(status) => Some(status),
            };
            self.event_hub.push(EventData::PaneAgentDetected {
                pane_id: pane_id.clone(),
                workspace_id: workspace_id.clone(),
                agent: update.agent_label.clone(),
                released: final_status.is_some(),
                final_status,
            });
        }
        let previous_status = pane_agent_status(update.previous_state, update.previous_seen);
        let status = pane_agent_status(
            update.state,
            workspace
                .pane_state(update.pane_id)
                .map_or(update.seen, |pane| pane.seen),
        );
        if previous_status != status || update.previous_presentation != update.presentation {
            let presentation = &update.presentation;
            self.event_hub.push(EventData::PaneAgentStatusChanged {
                pane_id,
                workspace_id,
                agent_status: status,
                agent: update.agent_label.clone(),
                title: presentation.title.clone(),
                display_agent: presentation.display_agent.clone(),
                state_labels: presentation.state_labels.clone(),
            });
        }
    }
}
