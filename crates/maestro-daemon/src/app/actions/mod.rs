//! Daemon-owned workspace mutations.

mod close;
mod death;
mod events;
mod focus;
mod managed;
mod metadata;
mod notifications;
mod ordering;
mod relocate;
mod removal;
mod state_update;
mod updates;
mod zoom;
pub use notifications::{
    active_tab_suppresses_notifications, is_completion_transition_parts,
    notification_sound_for_state_change, notification_toast_for_pane_state_update,
};
pub use relocate::PaneMoveRecovery;
pub use state_update::{AgentRelease, CompletionPolicy, PaneStateUpdate, pane_agent_status};

use maestro_workspace::{layout::PaneId, workspace::Workspace};

/// Formats a resolved workspace label, display position and optional tab label.
#[must_use]
pub fn notification_context(
    workspace: &Workspace,
    label: &str,
    index: usize,
    pane: PaneId,
) -> String {
    let mut context = format!("{label} · {}", index + 1);
    if workspace.tabs.len() > 1
        && let Some(tab) = workspace.find_tab_index_for_pane(pane)
        && let Some(label) = workspace.tab_display_name(tab)
    {
        context.push_str(" · ");
        context.push_str(&label);
    }
    context
}
