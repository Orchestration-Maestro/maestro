//! Background observations applied through the retained terminal-state owner.

use super::PaneStateUpdate;
use crate::{
    app::state::DaemonState,
    events::AppEvent,
    terminal::state::{DetectedStateUpdate, HookReport, TerminalState},
};
use maestro_detection::agents::AgentVisibility;
use maestro_workspace::layout::PaneId;

impl<C: FnMut() -> u128> DaemonState<C> {
    /// Applies background state observations; host bell and clipboard effects stay with the loop.
    pub fn handle_app_event(&mut self, event: AppEvent) -> Vec<PaneStateUpdate> {
        let update = match event {
            AppEvent::UpdateReady {
                version,
                install_command,
            } => {
                self.record_update_ready(version, install_command);
                None
            }
            AppEvent::AgentDetectionManifestsUpdated { updated, status } => {
                self.record_manifest_update(&updated, status);
                None
            }
            AppEvent::PaneDied { pane_id } => {
                self.handle_pane_died(pane_id);
                None
            }
            screen @ (AppEvent::AgentProcessDetected { .. } | AppEvent::StateChanged { .. }) => {
                self.handle_screen_event(&screen)
            }
            AppEvent::HookStateReported {
                pane_id,
                source,
                agent_label,
                state,
                message,
                seq,
                session_ref,
            } => self.update_hook_state(
                pane_id,
                HookReport {
                    source,
                    agent_label,
                    state,
                    message,
                    session_ref,
                    seq,
                },
            ),
            AppEvent::HookAgentReleased {
                pane_id,
                source,
                agent_label,
                seq,
                ..
            } if !crate::agent_resume::is_official_agent_source(&source, &agent_label) => self
                .update_terminal_state(pane_id, |terminal| {
                    terminal.release_agent_with_mutation(&source, &agent_label, seq)
                }),
            AppEvent::TerminalCwdReported { pane_id, cwd } => {
                self.update_terminal_cwd(pane_id, cwd);
                None
            }
            AppEvent::TerminalBell { .. }
            | AppEvent::ClipboardWrite { .. }
            | AppEvent::TabBarCommandFinished { .. }
            | AppEvent::WorktreeOperationFinished { .. }
            | AppEvent::HookAgentReleased { .. } => None,
        };
        update.into_iter().collect()
    }

    /// Reconciles process and screen observations through the terminal owner.
    fn handle_screen_event(&mut self, event: &AppEvent) -> Option<PaneStateUpdate> {
        match *event {
            AppEvent::AgentProcessDetected {
                pane_id,
                agent,
                observed_at,
            } => self.update_terminal_state(pane_id, |terminal| {
                Some(terminal.set_detected_agent_process_at(agent, observed_at))
            }),
            AppEvent::StateChanged {
                pane_id,
                agent,
                state,
                visible_blocker,
                visible_working,
                process_exited,
                observed_at,
            } => self.update_terminal_state(pane_id, |terminal| {
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
            }),
            _ => None,
        }
    }

    /// Reserves native state ownership while still accepting its session identity.
    fn update_hook_state(&mut self, pane: PaneId, report: HookReport) -> Option<PaneStateUpdate> {
        self.update_terminal_state(pane, |terminal| {
            if crate::agent_resume::is_reserved_native_state_source(
                &report.source,
                &report.agent_label,
            ) {
                terminal.set_agent_session_ref(
                    report.source,
                    report.agent_label,
                    report.session_ref,
                    report.seq,
                )
            } else {
                terminal.set_hook_authority_with_session_ref(report)
            }
        })
    }

    /// Retains only an existing absolute directory and marks changed sessions dirty.
    fn update_terminal_cwd(&mut self, pane: PaneId, cwd: std::path::PathBuf) {
        if cwd.is_absolute()
            && cwd.is_dir()
            && let Some(terminal) = self.terminal_for_pane_mut(pane)
            && terminal.cwd != cwd
        {
            terminal.cwd = cwd;
            self.session_dirty = true;
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
