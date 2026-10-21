//! Effective terminal changes projected for daemon consumers.

use crate::{
    app::state::DaemonState,
    terminal::{
        metadata::EffectivePresentation,
        state::{EffectiveStateChange, TerminalState, TerminalStateMutation},
    },
};
use maestro_detection::agents::{Agent, AgentState};
use maestro_protocol::schema::AgentStatus;
use maestro_workspace::layout::PaneId;
use std::time::Instant;

/// Whether acquisition allows a completion notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionPolicy {
    /// Apply ordinary completion decisions.
    Notify,
    /// Initial acquisition or managed startup is not completion.
    Suppress,
}

/// Ownership outcome of a terminal transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentRelease {
    /// Agent ownership was not released.
    Retained,
    /// Ownership was released with the resulting shared status.
    Released(AgentStatus),
}

/// Before and after observations for one pane transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneStateUpdate {
    /// Pane whose state changed.
    pub pane_id: PaneId,
    /// Workspace display position when applied.
    pub ws_idx: usize,
    /// Effective label before the transition.
    pub previous_agent_label: Option<String>,
    /// Known identity before the transition.
    pub previous_known_agent: Option<Agent>,
    /// Execution state before the transition.
    pub previous_state: AgentState,
    /// Attention acknowledgement before the transition.
    pub previous_seen: bool,
    /// Presentation before the transition.
    pub previous_presentation: EffectivePresentation,
    /// Effective or released agent label.
    pub agent_label: Option<String>,
    /// Effective or released known identity.
    pub known_agent: Option<Agent>,
    /// Resulting execution state.
    pub state: AgentState,
    /// Resulting attention acknowledgement.
    pub seen: bool,
    /// Resulting presentation.
    pub presentation: EffectivePresentation,
    /// Whether the retained agent name changed.
    pub agent_name_changed: bool,
    /// Ownership release and its associated shared status.
    pub release: AgentRelease,
    /// Whether initial acquisition or a managed launch suppresses completion.
    pub completion: CompletionPolicy,
}

impl PaneStateUpdate {
    /// Projects the effective change before applying pane-level acknowledgement.
    fn from_change(
        workspace: usize,
        pane: PaneId,
        seen: bool,
        change: EffectiveStateChange,
    ) -> Self {
        Self {
            pane_id: pane,
            ws_idx: workspace,
            previous_agent_label: change.previous_agent_label,
            previous_known_agent: change.previous_known_agent,
            previous_state: change.previous_state,
            previous_seen: seen,
            previous_presentation: change.previous_presentation,
            agent_label: change.agent_label,
            known_agent: change.known_agent,
            state: change.state,
            seen,
            presentation: change.presentation,
            agent_name_changed: false,
            release: AgentRelease::Retained,
            completion: CompletionPolicy::Notify,
        }
    }
}

impl<C: FnMut() -> u128> DaemonState<C> {
    /// Applies one terminal mutation and retains its observable transition metadata.
    pub(super) fn update_terminal_state(
        &mut self,
        pane: PaneId,
        update: impl FnOnce(&mut TerminalState) -> Option<TerminalStateMutation>,
    ) -> Option<PaneStateUpdate> {
        let workspace = self
            .topology
            .workspaces
            .iter()
            .position(|workspace| workspace.pane_state(pane).is_some())?;
        let viewport = self.topology.workspaces[workspace].pane_state(pane)?;
        let previous_seen = viewport.seen;
        let terminal_id = viewport.attached_terminal_id.clone();
        let terminal = self.terminals.get_mut(&terminal_id)?;
        let previous_name = terminal.agent_name.clone();
        let pending = terminal.managed_agent_launch_pending();
        let mutation = update(terminal)?;
        let now = Instant::now();
        let managed_changed = terminal.reconcile_managed_agent_at(now, false);
        let suppress = terminal.finish_agent_process_acquisition();
        let name_changed = terminal.agent_name != previous_name;
        self.session_dirty |= mutation.session_ref_changed || managed_changed || name_changed;
        let change = mutation.effective_state_change.or_else(|| {
            (mutation.agent_released || name_changed)
                .then(|| terminal.unchanged_effective_state_change_at(now))
        })?;
        let suppress_completion = change.state == AgentState::Idle && (pending || suppress);
        if change.previous_state != change.state {
            self.next_agent_state_change_seq += 1;
            terminal.last_agent_state_change_seq = Some(self.next_agent_state_change_seq);
        }
        let seen = self.apply_pane_state_change(workspace, pane, &change, suppress_completion)?;
        let mut result = PaneStateUpdate::from_change(workspace, pane, previous_seen, change);
        result.seen = seen;
        if suppress_completion {
            result.completion = CompletionPolicy::Suppress;
        }
        result.agent_name_changed = name_changed;
        if mutation.agent_released {
            result.agent_label.clone_from(&result.previous_agent_label);
            result.known_agent = result.previous_known_agent;
        }
        if mutation.agent_released {
            result.release = AgentRelease::Released(pane_agent_status(result.state, result.seen));
        }
        Some(result)
    }
}

/// Projects execution and acknowledgement into the shared agent status.
#[must_use]
pub fn pane_agent_status(state: AgentState, seen: bool) -> AgentStatus {
    match (state, seen) {
        (AgentState::Idle, false) => AgentStatus::Done,
        (AgentState::Idle, true) => AgentStatus::Idle,
        (AgentState::Working, _) => AgentStatus::Working,
        (AgentState::Blocked, _) => AgentStatus::Blocked,
        (AgentState::Unknown, _) => AgentStatus::Unknown,
    }
}
