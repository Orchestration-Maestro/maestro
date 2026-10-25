//! Render request coalescing at the request/drain interface.

use super::{HashSet, PaneId, RenderSignal};

#[test]
fn pty_damage_is_deduplicated_until_drain() {
    let signal = RenderSignal::new();
    let first = PaneId::from_raw(10);
    let second = PaneId::from_raw(20);

    assert!(!signal.is_pending());
    assert!(signal.request_pty(first));
    assert!(!signal.request_pty(first));
    assert!(!signal.request_pty(second));
    assert!(signal.is_pending());

    let request = signal.take();
    assert!(!request.generic);
    assert_eq!(request.pty_sources, HashSet::from([first, second]));
    assert!(request.terminal_title_sources.is_empty());
    assert!(!signal.is_pending());
    assert!(signal.request_pty(first));
}

#[test]
fn hidden_panes_share_one_pending_wakeup() {
    let signal = RenderSignal::new();
    signal.set_immediate_pty_sources(HashSet::from([PaneId::from_raw(100)]));

    let wakes = (1..=50)
        .filter(|id| signal.request_pty(PaneId::from_raw(*id)))
        .count();

    assert_eq!(wakes, 1);
    assert!(!signal.has_immediate_work());
    assert_eq!(signal.take().pty_sources.len(), 50);
}

#[test]
fn visible_damage_interrupts_hidden_pending_work() {
    let signal = RenderSignal::new();
    let hidden = PaneId::from_raw(10);
    let visible = PaneId::from_raw(20);
    signal.set_immediate_pty_sources(HashSet::from([visible]));

    assert!(signal.request_pty(hidden));
    assert!(!signal.has_immediate_work());
    assert!(!signal.request_pty(PaneId::from_raw(30)));
    assert!(signal.request_pty(visible));
    assert!(!signal.request_pty(visible));
    assert!(signal.has_immediate_work());
    signal.set_immediate_pty_sources(HashSet::new());
    assert!(!signal.has_immediate_work());
    signal.set_immediate_pty_sources(HashSet::from([visible]));
    assert!(signal.has_immediate_work());
    let _ = signal.take();
    assert!(!signal.has_immediate_work());
}

#[test]
fn new_title_interrupts_pending_damage() {
    let signal = RenderSignal::new();
    let pane = PaneId::from_raw(10);

    assert!(signal.request_pty(pane));
    assert!(signal.request_terminal_title(pane));
    assert!(!signal.request_terminal_title(pane));
    assert!(signal.has_immediate_work());
    assert_eq!(signal.take().terminal_title_sources, HashSet::from([pane]));
    assert!(!signal.has_immediate_work());
}

#[test]
fn title_changes_preserve_metadata_only_origins() {
    let signal = RenderSignal::new();
    let pane = PaneId::from_raw(10);
    let second = PaneId::from_raw(20);

    assert!(signal.request_terminal_title(pane));
    assert!(!signal.request_terminal_title(pane));
    assert!(signal.request_terminal_title(second));
    assert_eq!(
        signal.pending_terminal_title_sources(),
        HashSet::from([pane, second])
    );

    let request = signal.take();
    assert!(request.pty_sources.is_empty());
    assert!(!request.generic);
    assert_eq!(
        request.terminal_title_sources,
        HashSet::from([pane, second])
    );
    assert!(signal.pending_terminal_title_sources().is_empty());
}

#[test]
fn generic_requests_survive_pty_coalescing() {
    let signal = RenderSignal::new();
    let pane = PaneId::from_raw(10);

    signal.request_generic();
    assert!(signal.is_pending());
    assert!(signal.has_immediate_work());
    assert!(!signal.request_pty(pane));

    let request = signal.take();
    assert!(request.generic);
    assert_eq!(request.pty_sources, HashSet::from([pane]));
    assert!(!signal.has_immediate_work());
    let empty = signal.take();
    assert!(!empty.generic);
    assert!(empty.pty_sources.is_empty());
    assert!(empty.terminal_title_sources.is_empty());
}
