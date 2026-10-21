//! Background observations applied through the retained terminal-state owner.

use super::PaneStateUpdate;
use crate::{
    app::state::DaemonState,
    events::AppEvent,
    terminal::state::{DetectedStateUpdate, TerminalState},
};
use maestro_detection::agents::AgentVisibility;
use maestro_workspace::layout::PaneId;

impl<C: FnMut() -> u128> DaemonState<C> {
    /// Applies background state observations; host bell and clipboard effects stay with the loop.
    pub fn handle_app_event(&mut self, event: AppEvent) -> Vec<PaneStateUpdate> {
        match event {
            AppEvent::PaneDied { pane_id } => {
                self.handle_pane_died(pane_id);
                Vec::new()
            }
            AppEvent::AgentProcessDetected {
                pane_id,
                agent,
                observed_at,
            } => self
                .update_terminal_state(pane_id, |terminal| {
                    Some(terminal.set_detected_agent_process_at(agent, observed_at))
                })
                .into_iter()
                .collect(),
            AppEvent::StateChanged {
                pane_id,
                agent,
                state,
                visible_blocker,
                visible_working,
                process_exited,
                observed_at,
            } => self
                .update_terminal_state(pane_id, |terminal| {
                    Some(terminal.set_detected_state_with_screen_signals_at(
                        DetectedStateUpdate {
                            agent,
                            fallback_state: state,
                            visible: AgentVisibility {
                                visible_blocker,
                                visible_idle: false,
                                visible_working,
                            },
                            process_exited,
                        },
                        observed_at,
                    ))
                })
                .into_iter()
                .collect(),
            AppEvent::TerminalCwdReported { pane_id, cwd } => {
                if cwd.is_absolute()
                    && cwd.is_dir()
                    && let Some(terminal) = self.terminal_for_pane_mut(pane_id)
                    && terminal.cwd != cwd
                {
                    terminal.cwd = cwd;
                    self.session_dirty = true;
                }
                Vec::new()
            }
            AppEvent::TerminalBell { .. } | AppEvent::ClipboardWrite { .. } => Vec::new(),
        }
    }

    /// Borrows metadata through the pane's attached terminal identity.
    fn terminal_for_pane_mut(&mut self, pane: PaneId) -> Option<&mut TerminalState> {
        let terminal = self
            .topology
            .workspaces
            .iter()
            .find_map(|workspace| workspace.terminal_id(pane))?;
        self.terminals.get_mut(terminal)
    }
}
