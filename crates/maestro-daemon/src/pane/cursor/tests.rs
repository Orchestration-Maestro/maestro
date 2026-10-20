//! Cursor settling with supplied time observations.

use super::*;
use crate::pane::terminal::TerminalCursorState;
use std::time::{Duration, Instant};

fn cursor(x: u16, y: u16, visible: bool, shape: u8) -> TerminalCursorState {
    TerminalCursorState {
        x,
        y,
        visible,
        shape,
    }
}

#[test]
fn pane_cursor_settle_holds_position_change_until_quiet_window() {
    let now = Instant::now();
    let mut settle = CursorPositionSettleState::default();
    settle.observe(Some(cursor(1, 0, true, 0)), now);
    settle.observe(Some(cursor(20, 5, true, 0)), now + Duration::from_millis(1));

    let reported = settle
        .reported_cursor(Some(cursor(20, 5, true, 0)), now + Duration::from_millis(2))
        .unwrap();

    assert_eq!((reported.x, reported.y), (1, 0));
}

#[test]
fn pane_cursor_settle_adopts_position_change_after_quiet_window() {
    let now = Instant::now();
    let mut settle = CursorPositionSettleState::default();
    settle.observe(Some(cursor(1, 0, true, 0)), now);
    settle.observe(Some(cursor(2, 0, true, 0)), now + Duration::from_millis(1));

    let reported = settle
        .reported_cursor(
            Some(cursor(2, 0, true, 0)),
            now + CURSOR_POSITION_SETTLE + Duration::from_millis(1),
        )
        .unwrap();

    assert_eq!((reported.x, reported.y), (2, 0));
}

#[test]
fn pane_cursor_settle_caps_continuous_position_changes_from_first_pending_time() {
    let now = Instant::now();
    let mut settle = CursorPositionSettleState::default();
    settle.observe(Some(cursor(1, 0, true, 0)), now);
    settle.observe(Some(cursor(2, 0, true, 0)), now + Duration::from_millis(1));
    settle.observe(
        Some(cursor(3, 0, true, 0)),
        now + CURSOR_POSITION_MAX_HOLD + Duration::from_millis(1),
    );

    assert!(!settle.pending());
    assert_eq!(
        settle.reported_cursor(
            Some(cursor(3, 0, true, 0)),
            now + CURSOR_POSITION_MAX_HOLD + Duration::from_millis(2),
        ),
        Some(cursor(3, 0, true, 0))
    );
}

#[test]
fn pane_cursor_settle_keeps_render_read_pure() {
    let now = Instant::now();
    let mut settle = CursorPositionSettleState::default();
    settle.observe(Some(cursor(1, 0, true, 0)), now);
    settle.observe(Some(cursor(2, 0, true, 0)), now + Duration::from_millis(1));

    assert!(settle.pending());
    let _ = settle.reported_cursor(
        Some(cursor(2, 0, true, 0)),
        now + CURSOR_POSITION_SETTLE + Duration::from_millis(1),
    );

    assert!(settle.pending());
}

#[test]
fn pane_cursor_settle_passes_shape_through_while_position_is_held() {
    let now = Instant::now();
    let mut settle = CursorPositionSettleState::default();
    settle.observe(Some(cursor(1, 0, true, 2)), now);
    settle.observe(Some(cursor(2, 0, true, 6)), now + Duration::from_millis(1));

    let reported = settle
        .reported_cursor(Some(cursor(2, 0, true, 6)), now + Duration::from_millis(2))
        .unwrap();

    assert_eq!((reported.x, reported.y, reported.shape), (1, 0, 6));
}

#[test]
fn pane_cursor_settle_passes_shape_through_after_quiet_window() {
    let now = Instant::now();
    let mut settle = CursorPositionSettleState::default();
    settle.observe(Some(cursor(1, 0, true, 2)), now);
    settle.observe(Some(cursor(2, 0, true, 2)), now + Duration::from_millis(1));

    let reported = settle
        .reported_cursor(
            Some(cursor(2, 0, true, 6)),
            now + CURSOR_POSITION_SETTLE + Duration::from_millis(1),
        )
        .unwrap();

    assert_eq!((reported.x, reported.y, reported.shape), (2, 0, 6));
}

#[test]
fn pane_cursor_settle_hides_immediately_and_waits_to_reveal() {
    let now = Instant::now();
    let mut settle = CursorPositionSettleState::default();
    settle.observe(Some(cursor(1, 0, true, 0)), now);
    settle.observe(Some(cursor(1, 0, false, 0)), now + Duration::from_millis(1));

    assert_eq!(
        settle.reported_cursor(Some(cursor(1, 0, false, 0)), now + Duration::from_millis(2)),
        Some(cursor(1, 0, false, 0))
    );

    settle.observe(Some(cursor(1, 0, true, 0)), now + Duration::from_millis(3));
    assert_eq!(
        settle.reported_cursor(Some(cursor(1, 0, true, 0)), now + Duration::from_millis(4)),
        Some(cursor(1, 0, false, 0))
    );
}

#[test]
fn pane_cursor_observations_commit_stable_candidates_and_cancel_on_return() {
    let now = Instant::now();
    let first = Some(cursor(1, 0, true, 2));
    let second = Some(cursor(2, 0, true, 6));
    let third = Some(cursor(3, 0, true, 6));
    let mut settle = CursorPositionSettleState::default();
    settle.observe(first, now);
    settle.observe(second, now + Duration::from_millis(1));
    settle.observe(second, now + Duration::from_millis(2));
    assert_eq!(
        settle.reported_cursor(second, now + Duration::from_millis(2)),
        first.map(|c| TerminalCursorState { shape: 6, ..c })
    );
    settle.observe(third, now + Duration::from_millis(3));
    assert!(settle.pending());
    settle.observe(first, now + Duration::from_millis(4));
    assert!(!settle.pending());
    settle.observe(second, now + Duration::from_millis(5));
    settle.observe(second, now + Duration::from_millis(25));
    assert!(!settle.pending());
    assert_eq!(
        settle.reported_cursor(second, now + Duration::from_millis(25)),
        second
    );
}

#[test]
fn pane_cursor_absence_clears_a_pending_move() {
    let now = Instant::now();
    let mut settle = CursorPositionSettleState::default();
    settle.observe(Some(cursor(1, 0, true, 0)), now);
    settle.observe(Some(cursor(2, 0, true, 0)), now);
    settle.observe(None, now);
    assert!(!settle.pending());
    assert_eq!(settle.reported_cursor(None, now), None);
    let current = Some(cursor(8, 2, true, 4));
    settle.observe(current, now);
    assert_eq!(settle.reported_cursor(current, now), current);
}
