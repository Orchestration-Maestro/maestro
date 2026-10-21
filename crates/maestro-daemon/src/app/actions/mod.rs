//! Daemon-owned workspace mutations.

mod ordering;

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
