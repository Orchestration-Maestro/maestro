//! Split-tree operations.
use super::{NavDirection, PaneId, PaneInfo, Rect, SplitBorder, SplitDirection, ranges_overlap};
use std::cmp::Reverse;

/// Test whether a divider meets the requested pane edge.
fn split_on_requested_edge(split: &SplitBorder, focused: Rect, nav: NavDirection) -> bool {
    split_edge_distance(split, focused, nav) <= 1
}
/// Test perpendicular overlap with the divider region.
fn split_area_overlaps_focused_pane(split: &SplitBorder, focused: Rect, nav: NavDirection) -> bool {
    match nav {
        NavDirection::Left | NavDirection::Right => {
            ranges_overlap(split.area.y, split.area.height, focused.y, focused.height)
        }
        NavDirection::Up | NavDirection::Down => {
            ranges_overlap(split.area.x, split.area.width, focused.x, focused.width)
        }
    }
}
/// Select the nearest eligible divider in traversal order.
pub(super) fn nearest_resize_split<'a>(
    splits: &'a [SplitBorder],
    target_dir: &SplitDirection,
    focused: Rect,
    nav: NavDirection,
) -> Option<&'a SplitBorder> {
    splits
        .iter()
        .filter(|s| &s.direction == target_dir)
        .filter(|s| split_area_overlaps_focused_pane(s, focused, nav))
        .filter(|s| split_on_requested_edge(s, focused, nav))
        .min_by_key(|s| split_edge_distance(s, focused, nav))
}
/// Reverse a cardinal direction.
pub(super) fn opposite_direction(nav: NavDirection) -> NavDirection {
    match nav {
        NavDirection::Left => NavDirection::Right,
        NavDirection::Right => NavDirection::Left,
        NavDirection::Up => NavDirection::Down,
        NavDirection::Down => NavDirection::Up,
    }
}
/// Measure divider distance from the requested edge.
fn split_edge_distance(split: &SplitBorder, focused: Rect, nav: NavDirection) -> u32 {
    match nav {
        NavDirection::Left => (i32::from(split.pos) - i32::from(focused.x)).unsigned_abs(),
        NavDirection::Right => (i32::from(split.pos) - i32::from(focused.right())).unsigned_abs(),
        NavDirection::Up => (i32::from(split.pos) - i32::from(focused.y)).unsigned_abs(),
        NavDirection::Down => (i32::from(split.pos) - i32::from(focused.bottom())).unsigned_abs(),
    }
}

/// Find a neighbour by edge distance, overlap, center distance and tree order.
#[must_use]
pub fn find_in_direction(
    focused: &PaneInfo,
    direction: NavDirection,
    panes: &[PaneInfo],
) -> Option<PaneId> {
    let fr = focused.rect;

    panes
        .iter()
        .enumerate()
        .filter(|(_, p)| p.id != focused.id)
        .filter(|(_, p)| {
            let r = p.rect;
            match direction {
                NavDirection::Left => {
                    r.right() <= fr.x && ranges_overlap(r.y, r.height, fr.y, fr.height)
                }
                NavDirection::Right => {
                    r.x >= fr.right() && ranges_overlap(r.y, r.height, fr.y, fr.height)
                }
                NavDirection::Up => {
                    r.bottom() <= fr.y && ranges_overlap(r.x, r.width, fr.x, fr.width)
                }
                NavDirection::Down => {
                    r.y >= fr.bottom() && ranges_overlap(r.x, r.width, fr.x, fr.width)
                }
            }
        })
        .min_by_key(|(index, p)| {
            let r = p.rect;
            let edge_distance = match direction {
                NavDirection::Left => fr.x.saturating_sub(r.right()),
                NavDirection::Right => r.x.saturating_sub(fr.right()),
                NavDirection::Up => fr.y.saturating_sub(r.bottom()),
                NavDirection::Down => r.y.saturating_sub(fr.bottom()),
            };
            let overlap = match direction {
                NavDirection::Left | NavDirection::Right => {
                    range_overlap_amount(r.y, r.height, fr.y, fr.height)
                }
                NavDirection::Up | NavDirection::Down => {
                    range_overlap_amount(r.x, r.width, fr.x, fr.width)
                }
            };
            let center_distance = match direction {
                NavDirection::Left | NavDirection::Right => {
                    range_center_distance(r.y, r.height, fr.y, fr.height)
                }
                NavDirection::Up | NavDirection::Down => {
                    range_center_distance(r.x, r.width, fr.x, fr.width)
                }
            };
            (edge_distance, Reverse(overlap), center_distance, *index)
        })
        .map(|(_, p)| p.id)
}
/// Compute the perpendicular shared extent.
fn range_overlap_amount(a_start: u16, a_len: u16, b_start: u16, b_len: u16) -> u16 {
    let a_end = a_start.saturating_add(a_len);
    let b_end = b_start.saturating_add(b_len);
    a_end.min(b_end).saturating_sub(a_start.max(b_start))
}
/// Compute distance between doubled interval centers.
fn range_center_distance(a_start: u16, a_len: u16, b_start: u16, b_len: u16) -> u16 {
    let a_center = a_start.saturating_mul(2).saturating_add(a_len);
    let b_center = b_start.saturating_mul(2).saturating_add(b_len);
    a_center.abs_diff(b_center)
}
