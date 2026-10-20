//! Selection lifecycle and scroll-coordinate witnesses.

use super::*;
use crate::pane::terminal::ScrollMetrics;
use maestro_workspace::layout::{PaneId, Rect};

fn make_sel(sr: u32, sc: u16, er: u32, ec: u16) -> Selection {
    let metrics = |row| {
        Some(ScrollMetrics {
            offset_from_bottom: 0,
            max_offset_from_bottom: usize::try_from(row).unwrap(),
            viewport_rows: 1,
        })
    };
    let mut selection = Selection::anchor(PaneId::from_raw(0), 0, sc, metrics(sr));
    selection.drag(ec, 0, Rect::new(0, 0, u16::MAX, 1), metrics(er));
    selection.force_dragging();
    selection
}

#[test]
fn selection_ordering_forward() {
    let sel = make_sel(2, 5, 4, 10);
    assert_eq!(sel.ordered_cells(), ((2, 5), (4, 10)));
}

#[test]
fn selection_ordering_backward() {
    let sel = make_sel(4, 10, 2, 5);
    assert_eq!(sel.ordered_cells(), ((2, 5), (4, 10)));
}

#[test]
fn selection_single_line_contains() {
    let sel = make_sel(2, 5, 2, 15);
    assert!(!sel.contains(2, 4, None));
    assert!(sel.contains(2, 5, None));
    assert!(sel.contains(2, 10, None));
    assert!(sel.contains(2, 15, None));
    assert!(!sel.contains(2, 16, None));
    assert!(!sel.contains(1, 10, None));
    assert!(!sel.contains(3, 10, None));
}

#[test]
fn selection_multi_line_contains() {
    let sel = make_sel(2, 5, 4, 10);
    assert!(!sel.contains(2, 4, None));
    assert!(sel.contains(2, 5, None));
    assert!(sel.contains(2, 79, None));
    assert!(sel.contains(3, 0, None));
    assert!(sel.contains(3, 79, None));
    assert!(sel.contains(4, 0, None));
    assert!(sel.contains(4, 10, None));
    assert!(!sel.contains(4, 11, None));
}

#[test]
fn selection_anchored_not_visible() {
    let sel = Selection::anchor(PaneId::from_raw(0), 5, 10, None);
    assert!(!sel.is_visible());
    assert!(!sel.contains(5, 10, None));
}

#[test]
fn selection_click_without_drag() {
    let mut sel = Selection::anchor(PaneId::from_raw(0), 5, 10, None);
    assert!(sel.was_just_click());
    let copied = sel.finish();
    assert!(!copied);
}

#[test]
fn selection_drag_then_finish() {
    let mut sel = Selection::anchor(PaneId::from_raw(0), 5, 10, None);
    sel.drag(20, 7, Rect::new(10, 5, 80, 24), None);
    assert!(sel.is_visible());
    assert!(!sel.was_just_click());
    let copied = sel.finish();
    assert!(copied);
}

#[test]
fn selection_drag_uses_buffer_rows_when_scrolled() {
    let mut sel = Selection::anchor(
        PaneId::from_raw(0),
        0,
        10,
        Some(ScrollMetrics {
            offset_from_bottom: 1,
            max_offset_from_bottom: 10,
            viewport_rows: 4,
        }),
    );

    sel.drag(
        10,
        5,
        Rect::new(10, 5, 80, 4),
        Some(ScrollMetrics {
            offset_from_bottom: 2,
            max_offset_from_bottom: 10,
            viewport_rows: 4,
        }),
    );

    assert_eq!(sel.ordered_cells(), ((8, 0), (9, 10)));
}

#[test]
fn selection_contains_tracks_current_viewport_after_scroll() {
    let sel = make_sel(8, 2, 10, 4);
    let metrics = Some(ScrollMetrics {
        offset_from_bottom: 2,
        max_offset_from_bottom: 10,
        viewport_rows: 4,
    });

    assert!(sel.contains(0, 2, metrics));
    assert!(sel.contains(1, 40, metrics));
    assert!(sel.contains(2, 4, metrics));
    assert!(!sel.contains(3, 4, metrics));
}

#[test]
fn selection_clamp_to_pane_bounds() {
    let mut selection = Selection::anchor(PaneId::from_raw(0), 0, 0, None);
    selection.drag(200, 100, Rect::new(10, 5, 80, 24), None);
    assert_eq!(selection.ordered_cells(), ((0, 0), (23, 79)));

    let mut selection = Selection::anchor(PaneId::from_raw(0), 23, 79, None);
    selection.drag(0, 0, Rect::new(10, 5, 80, 24), None);
    assert_eq!(selection.ordered_cells(), ((0, 0), (23, 79)));
}

#[test]
fn selection_anchor_screen_pos_adds_pane_origin() {
    // Pane offset by sidebar (x=10) and tab bar (y=5).
    // Anchor at viewport_row=3, col=5 (pane-relative).
    let sel = Selection::anchor(PaneId::from_raw(0), 3, 5, None);
    let pane_inner = Rect::new(10, 5, 80, 24);
    let (row, col) = sel.anchor_screen_pos(pane_inner, None);
    // Screen row = 3 + 5 = 8, screen col = 5 + 10 = 15
    assert_eq!(row, 8);
    assert_eq!(col, 15);
}

#[test]
fn selection_anchor_screen_pos_same_cell_as_mouse_with_offset() {
    // When the pane has a non-zero origin, anchor and mouse on the same
    // screen cell must compare equal — no false drag detection.
    let pane_inner = Rect::new(10, 5, 80, 24);
    // Mouse clicked at screen (15, 8) → anchor stored as (viewport_row=3, col=5)
    let sel = Selection::anchor(PaneId::from_raw(0), 3, 5, None);
    let (ar, ac) = sel.anchor_screen_pos(pane_inner, None);
    // Screen position of the anchor must match the mouse position
    assert_eq!((ar, ac), (8, 15));
}

#[test]
fn selection_explicit_ranges_preserve_rows_and_drag_direction() {
    let pane = PaneId::from_raw(7);
    let metrics = Some(ScrollMetrics {
        offset_from_bottom: 2,
        max_offset_from_bottom: 10,
        viewport_rows: 4,
    });
    let selection = Selection::range(pane, 2, 8, 3, metrics);
    assert_eq!(selection.pane_id, pane);
    assert_eq!(selection.ordered_cells(), ((10, 3), (10, 8)));
    assert!(selection.is_dragging());
    assert_eq!(Selection::absolute_row_for_viewport(2, metrics), 10);

    for (anchor, cursor) in [(8, 10), (10, 8)] {
        let selection = Selection::line_range(pane, anchor, cursor, 79);
        assert_eq!(selection.ordered_cells(), ((8, 0), (10, 79)));
        assert!(selection.contains(1, 40, metrics));
    }
}

#[test]
fn selection_forced_drag_finishes_without_reopening() {
    let mut selection = Selection::anchor(PaneId::from_raw(0), 0, 0, None);
    assert!(selection.is_just_click());
    assert!(selection.is_in_progress());
    selection.force_dragging();
    assert!(selection.is_dragging());
    assert!(selection.finish());
    selection.force_dragging();
    assert!(selection.is_finalized());
    assert!(!selection.is_in_progress());
    assert!(!selection.finish());
}

#[test]
fn selection_anchor_clamps_after_viewport_changes() {
    let pane = Rect::new(10, 5, 80, 4);
    let selection = Selection::anchor(PaneId::from_raw(0), 5, 100, None);
    assert_eq!(selection.anchor_screen_pos(pane, None), (8, 89));
    let metrics = Some(ScrollMetrics {
        offset_from_bottom: 0,
        max_offset_from_bottom: 8,
        viewport_rows: 4,
    });
    assert_eq!(selection.anchor_screen_pos(pane, metrics), (5, 89));
}
