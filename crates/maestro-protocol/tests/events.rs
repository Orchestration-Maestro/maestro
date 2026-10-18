//! Event name compatibility.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
#![cfg(test)]

use maestro_protocol::events::*;

#[test]
/// Known event names stay in sync with event kind.
fn event_kinds_match_the_independent_name_inventory() {
    let mut from_kind = KNOWN_EVENT_KINDS
        .iter()
        .map(|kind| kind.dot_name())
        .collect::<Vec<_>>();
    from_kind.sort_unstable();
    let mut known = known_event_names();
    known.sort_unstable();
    assert_eq!(
        from_kind, known,
        "known_event_names() out of sync with EventKind"
    );
}

#[test]
/// Action hook event names exclude high volume events.
fn action_hooks_filter_frequent_updates_but_keep_pane_moves() {
    let names = action_hook_event_names();
    assert!(!names.contains(&"pane.output_changed"));
    assert!(!names.contains(&"layout.updated"));
    assert!(!names.contains(&"workspace.metadata_updated"));
    assert!(!names.contains(&"pane.updated"));
    assert!(names.contains(&"pane.moved"));
}

/// Known event kinds.
const KNOWN_EVENT_KINDS: &[EventKind] = &[
    EventKind::WorkspaceCreated,
    EventKind::WorkspaceUpdated,
    EventKind::WorkspaceMetadataUpdated,
    EventKind::WorkspaceClosed,
    EventKind::WorkspaceRenamed,
    EventKind::WorkspaceMoved,
    EventKind::WorkspaceReordered,
    EventKind::WorkspaceFocused,
    EventKind::WorktreeCreated,
    EventKind::WorktreeOpened,
    EventKind::WorktreeRemoved,
    EventKind::TabCreated,
    EventKind::TabClosed,
    EventKind::TabRenamed,
    EventKind::TabMoved,
    EventKind::TabFocused,
    EventKind::PaneCreated,
    EventKind::PaneClosed,
    EventKind::PaneUpdated,
    EventKind::PaneFocused,
    EventKind::PaneMoved,
    EventKind::PaneOutputChanged,
    EventKind::PaneExited,
    EventKind::PaneAgentDetected,
    EventKind::PaneAgentStatusChanged,
    EventKind::LayoutUpdated,
];

/// Known event names.
fn known_event_names() -> Vec<&'static str> {
    vec![
        "workspace.created",
        "workspace.updated",
        "workspace.metadata_updated",
        "workspace.closed",
        "workspace.renamed",
        "workspace.moved",
        "workspace.reordered",
        "workspace.focused",
        "worktree.created",
        "worktree.opened",
        "worktree.removed",
        "tab.created",
        "tab.closed",
        "tab.renamed",
        "tab.moved",
        "tab.focused",
        "pane.created",
        "pane.closed",
        "pane.updated",
        "pane.focused",
        "pane.moved",
        "pane.output_changed",
        "pane.exited",
        "pane.agent_detected",
        "pane.agent_status_changed",
        "layout.updated",
    ]
}
