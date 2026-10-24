//! Attention acknowledgement and deferred agent-notification delivery.

use super::{
    notification_context,
    state_update::{CompletionPolicy, PaneStateUpdate},
};
use crate::{
    app::model::{
        AgentNotificationDelivery, DaemonState, PendingAgentNotification, ToastKind,
        ToastNotification, ToastTarget,
    },
    settings::model::{MAX_TOAST_DELAY_SECONDS, ToastDelivery},
    terminal::model::EffectiveStateChange,
};
use maestro_detection::agents::AgentState;
use maestro_protocol::schema::NotificationShowSound;
use maestro_workspace::tiling::PaneId;
use std::time::{Duration, Instant};

/// Whether execution changed from active work or established unknown ownership to idle.
#[must_use]
pub fn is_completion_transition_parts(
    previous: AgentState,
    state: AgentState,
    previous_label: Option<&str>,
    label: Option<&str>,
) -> bool {
    state == AgentState::Idle
        && (matches!(previous, AgentState::Working | AgentState::Blocked)
            || (previous == AgentState::Unknown
                && previous_label.is_some()
                && previous_label == label))
}

/// Whether an active tab suppresses client attention at the known outer focus state.
#[must_use]
pub fn active_tab_suppresses_notifications(active: bool, outer_focus: Option<bool>) -> bool {
    active && outer_focus != Some(false)
}

/// Selects notification sound without agent-label completion evidence.
#[must_use]
pub fn notification_sound_for_state_change(
    suppressed: bool,
    previous: AgentState,
    state: AgentState,
) -> Option<NotificationShowSound> {
    transition_kind(previous, state, None, None)
        .and_then(|kind| sound_for_toast_kind(kind, suppressed))
}

impl<C: FnMut() -> u128> DaemonState<C> {
    /// Whether a pane belongs to the active workspace's selected tab.
    #[must_use]
    pub fn pane_is_in_active_tab(&self, workspace: usize, pane: PaneId) -> bool {
        self.active == Some(workspace)
            && self
                .topology
                .workspaces
                .get(workspace)
                .is_some_and(|workspace| {
                    workspace.find_tab_index_for_pane(pane) == Some(workspace.active_tab)
                })
    }

    /// Applies attention state before retaining or delivering a notification.
    pub(super) fn apply_pane_state_change(
        &mut self,
        workspace: usize,
        pane: PaneId,
        change: &EffectiveStateChange,
        suppress: bool,
    ) -> Option<bool> {
        let suppressed = active_tab_suppresses_notifications(
            self.pane_is_in_active_tab(workspace, pane),
            self.outer_terminal_focus,
        );
        let viewport = self.topology.workspaces[workspace].pane_state_mut(pane)?;
        if change.state != AgentState::Idle {
            viewport.seen = true;
        } else if !suppress
            && is_completion_transition_parts(
                change.previous_state,
                change.state,
                change.previous_agent_label.as_deref(),
                change.agent_label.as_deref(),
            )
        {
            viewport.seen = suppressed;
        }
        let seen = viewport.seen;
        if !suppress
            && let Some(delivery) =
                self.record_or_deliver_agent_notification(workspace, pane, change)
        {
            self.apply_agent_notification_delivery(&delivery);
        }
        Some(seen)
    }

    /// Replaces pending evidence and schedules only a new attention transition.
    fn record_or_deliver_agent_notification(
        &mut self,
        workspace: usize,
        pane: PaneId,
        change: &EffectiveStateChange,
    ) -> Option<AgentNotificationDelivery> {
        self.pending_agent_notifications.remove(&pane);
        let suppressed = active_tab_suppresses_notifications(
            self.pane_is_in_active_tab(workspace, pane),
            self.outer_terminal_focus,
        );
        let kind = transition_kind(
            change.previous_state,
            change.state,
            change.previous_agent_label.as_deref(),
            change.agent_label.as_deref(),
        )?;
        if suppressed && sound_for_toast_kind(kind, true).is_none() {
            return None;
        }
        let now = Instant::now();
        let pending = PendingAgentNotification {
            pane_id: pane,
            workspace_id: self.topology.workspaces[workspace].id.clone(),
            agent_label: change
                .agent_label
                .as_ref()
                .or(change.previous_agent_label.as_ref())?
                .clone(),
            known_agent: change.known_agent.or(change.previous_known_agent),
            kind,
            state: change.state,
            deadline: now
                .checked_add(Duration::from_secs(
                    self.toast_config.delay_seconds.min(MAX_TOAST_DELAY_SECONDS),
                ))
                .unwrap_or(now),
        };
        if self.toast_config.delay_seconds == 0 {
            self.agent_notification_delivery(workspace, &pending)
        } else {
            self.pending_agent_notifications.insert(pane, pending);
            None
        }
    }

    /// Revalidates execution, identity and focus using live metadata at delivery time.
    fn agent_notification_delivery(
        &self,
        workspace: usize,
        pending: &PendingAgentNotification,
    ) -> Option<AgentNotificationDelivery> {
        let target = self.topology.workspaces.get(workspace)?;
        let terminal = self.terminals.get(target.terminal_id(pending.pane_id)?)?;
        if terminal.state != pending.state
            || terminal
                .effective_agent_label()
                .is_some_and(|label| label != pending.agent_label)
        {
            return None;
        }
        let active = self.pane_is_in_active_tab(workspace, pending.pane_id);
        let suppressed = active_tab_suppresses_notifications(active, self.outer_terminal_focus);
        let sound = sound_for_toast_kind(pending.kind, suppressed)
            .filter(|_| self.sound.allows(pending.known_agent));
        if active && suppressed && sound.is_none() {
            return None;
        }
        let label = self.workspace_label_from_terminals(workspace)?;
        let toast = ToastNotification {
            kind: pending.kind,
            title: format!(
                "{} {}",
                pending.agent_label,
                match pending.kind {
                    ToastKind::NeedsAttention => "needs attention",
                    ToastKind::Finished => "finished",
                    ToastKind::UpdateInstalled => "updated",
                }
            ),
            context: notification_context(target, &label, workspace, pending.pane_id),
            position: None,
            target: Some(ToastTarget {
                workspace_id: pending.workspace_id.clone(),
                pane_id: pending.pane_id,
            }),
        };
        Some(AgentNotificationDelivery {
            pane_id: pending.pane_id,
            workspace_id: pending.workspace_id.clone(),
            agent_label: pending.agent_label.clone(),
            known_agent: pending.known_agent,
            kind: pending.kind,
            toast: (!active).then(|| toast.clone()),
            client_notification: (!suppressed).then_some(toast),
            sound,
        })
    }

    /// Applies only workspace delivery; client notification and sound effects stay with the loop.
    fn apply_agent_notification_delivery(&mut self, delivery: &AgentNotificationDelivery) {
        if self.toast_config.delivery == ToastDelivery::Workspace
            && let Some(toast) = &delivery.toast
        {
            self.toast = Some(toast.clone());
        }
    }

    /// Resolves the first tab's current terminal directory through shared workspace labels.
    pub fn workspace_label_from_terminals(&self, workspace: usize) -> Option<String> {
        let workspace = self.topology.workspaces.get(workspace)?;
        Some(crate::workspace::display_name_from_terminals(
            workspace,
            &self.terminals,
            None,
        ))
    }

    /// Earliest pending attention deadline, absent when no transition is waiting.
    #[must_use]
    pub fn next_pending_agent_notification_deadline(&self) -> Option<Instant> {
        self.pending_agent_notifications
            .values()
            .map(|pending| pending.deadline)
            .min()
    }

    /// Drains due transitions, cancelling those whose live state or identity changed.
    pub fn drain_due_agent_notifications(
        &mut self,
        now: Instant,
    ) -> Vec<AgentNotificationDelivery> {
        let positions: std::collections::HashMap<_, _> = self
            .topology
            .workspaces
            .iter()
            .enumerate()
            .map(|(index, workspace)| (workspace.id.clone(), index))
            .collect();
        let due: Vec<_> = self
            .pending_agent_notifications
            .iter()
            .filter_map(|(pane, pending)| (pending.deadline <= now).then_some(*pane))
            .collect();
        due.into_iter()
            .filter_map(|pane| {
                let pending = self.pending_agent_notifications.remove(&pane)?;
                let workspace = *positions.get(&pending.workspace_id)?;
                let delivery = self.agent_notification_delivery(workspace, &pending)?;
                self.apply_agent_notification_delivery(&delivery);
                Some(delivery)
            })
            .collect()
    }
}

/// Whether a pane update permits a client notification.
#[must_use]
pub fn notification_toast_for_pane_state_update(
    suppressed: bool,
    update: &PaneStateUpdate,
) -> Option<ToastKind> {
    if update.completion == CompletionPolicy::Suppress
        || suppressed
        || update.state == update.previous_state
    {
        return None;
    }
    transition_kind(
        update.previous_state,
        update.state,
        update.previous_agent_label.as_deref(),
        update.agent_label.as_deref(),
    )
}

/// Classifies a new state transition independently of delivery destination.
fn transition_kind(
    previous: AgentState,
    state: AgentState,
    previous_label: Option<&str>,
    label: Option<&str>,
) -> Option<ToastKind> {
    if state == previous {
        return None;
    }
    match state {
        AgentState::Blocked => Some(ToastKind::NeedsAttention),
        AgentState::Idle
            if is_completion_transition_parts(previous, state, previous_label, label) =>
        {
            Some(ToastKind::Finished)
        }
        _ => None,
    }
}

/// Selects the sound independently of workspace notification visibility.
fn sound_for_toast_kind(kind: ToastKind, suppressed: bool) -> Option<NotificationShowSound> {
    match kind {
        ToastKind::NeedsAttention => Some(NotificationShowSound::Request),
        ToastKind::Finished if !suppressed => Some(NotificationShowSound::Done),
        ToastKind::Finished | ToastKind::UpdateInstalled => None,
    }
}
