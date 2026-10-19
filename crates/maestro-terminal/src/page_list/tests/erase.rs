//! Row erasure, managed ownership and viewport observations.

use super::resize::{integrity, screen};
use super::*;

/// Retain the original full-page history before exercising bulk erasure.
fn bulk_history() -> PageList {
    let mut list = screen(80, 24);
    assert_eq!(list.total_pages(), 1);
    let rows = usize::from(list.page(list.first).unwrap().capacity.rows);
    for _ in 0..rows * 5 {
        list.grow();
    }
    assert_eq!(list.total_pages(), 6);
    assert!(list.total_rows() > usize::from(list.rows()));
    list
}

/// Compare each retained slot's actual page and both coordinates.
fn locations(list: &PageList, expected: &[(TrackedPin, NodeId, u16, u16)]) {
    for &(handle, node, x, y) in expected {
        let pin = list.tracked_pin(handle).unwrap();
        assert_eq!((pin.node, pin.x, pin.y), (node, x, y));
    }
}

/// Check each row dirtied by the bounded shift, including its page-dirty parent.
fn dirty_rows(list: &PageList, rows: std::ops::Range<u32>) {
    for y in rows {
        assert_eq!(
            list.pin(Point::Active(Coordinate { x: 0, y }))
                .unwrap()
                .is_dirty(list),
            Some(true)
        );
    }
}

/// Retain single-page bounded erasure's source coordinates and dirty observations.
fn bounded_single(start: u32, limit: usize, pins: &[(u16, u16, u16, u16)]) {
    let mut list = screen(80, 10);
    let handles: Vec<_> = pins
        .iter()
        .map(|&(x, y, _, _)| {
            list.track_pin(
                list.pin(Point::Active(Coordinate { x, y: u32::from(y) }))
                    .unwrap(),
            )
            .unwrap()
        })
        .collect();
    list.erase_row_bounded(Point::Active(Coordinate { x: 0, y: start }), limit)
        .unwrap();
    assert_eq!(list.total_rows(), 10);
    dirty_rows(
        &list,
        start..(start + u32::try_from(limit).unwrap()).min(10),
    );
    for (handle, &(_, _, x, y)) in handles.into_iter().zip(pins) {
        locations(&list, &[(handle, list.first, x, y)]);
    }
    integrity(&list);
}

/// Retain the natural page boundary with five active rows in either page.
fn bounded_two_pages() {
    let mut list = screen(80, 10);
    let rows = list.page(list.first).unwrap().capacity.rows;
    for _ in 10..rows + 5 {
        list.grow();
    }
    assert_eq!(
        (list.total_pages(), list.page(list.last).unwrap().size.rows),
        (2, 5)
    );
    let handles = [4, 3, 8, 9].map(|y| {
        list.track_pin(list.pin(Point::Active(Coordinate { x: 0, y })).unwrap())
            .unwrap()
    });
    let [first, outside, inside, bottom] = handles;
    locations(
        &list,
        &[
            (first, list.first, 0, rows - 1),
            (outside, list.first, 0, rows - 2),
            (inside, list.last, 0, 3),
            (bottom, list.last, 0, 4),
        ],
    );
    list.erase_row_bounded(Point::Active(Coordinate { x: 0, y: 4 }), 4)
        .unwrap();
    dirty_rows(&list, 4..8);
    locations(
        &list,
        &[
            (first, list.first, 0, rows - 2),
            (outside, list.first, 0, rows - 2),
            (inside, list.last, 0, 2),
            (bottom, list.last, 0, 4),
        ],
    );
    integrity(&list);
}

#[test]
fn history_erase_row_bounded_less_than_full_row() {
    bounded_single(5, 3, &[(0, 5, 0, 4), (0, 8, 0, 7), (0, 9, 0, 9)]);
    let mut list = letter_rows("ABCDE", 3);
    let node = list.first;
    list.erase_row_bounded(Point::Screen(Coordinate { x: 0, y: 1 }), 1)
        .unwrap();
    assert_eq!(letters(&list), vec!['A', 'C', '\0', 'D', 'E']);
    assert!(list.node_is_valid(node));
    assert_eq!(list.total_rows(), 5);
}

#[test]
fn history_erase_active_regrows_automatically() {
    let mut source = screen(80, 24);
    assert_eq!(source.total_rows(), 24);
    source.erase_active(10);
    assert_eq!(source.total_rows(), 24);
    integrity(&source);
    let mut list = letter_rows("ABCDE", 5);
    list.erase_active(2);
    assert_eq!(letters(&list), vec!['D', 'E', '\0', '\0', '\0']);
    assert_eq!(list.total_rows(), 5);
}

/// Populate retained rows without filling unused capacity.
fn letter_rows(text: &str, active: u16) -> PageList {
    let mut list = PageList::init(
        NonZeroU16::new(3).unwrap(),
        NonZeroU16::new(active).unwrap(),
        None,
    );
    while list.total_rows() < text.len() {
        list.grow();
    }
    for (y, letter) in text.chars().enumerate() {
        let pin = list.pin_at_row(y, 0).unwrap();
        list.page_mut(pin.node).unwrap().get_cells_mut(pin.y)[0]
            .set_content(crate::page::cell::Content::Codepoint(letter));
    }
    list
}

/// Read each retained first-column cell in display order.
fn letters(list: &PageList) -> Vec<char> {
    let mut rows = list.row_iterator(
        Direction::RightDown,
        Point::Screen(Coordinate::default()),
        None,
    );
    std::iter::from_fn(|| rows.next(list))
        .map(|pin| pin.row_and_cell(list).unwrap().1.codepoint())
        .collect()
}

#[test]
fn history_erase_active_with_same_page_history_preserves_prefix() {
    let mut list = letter_rows("ABCDE", 3);
    let before = list.track_pin(list.pin_at_row(1, 1).unwrap()).unwrap();
    let removed = list.track_pin(list.pin_at_row(2, 1).unwrap()).unwrap();
    let after = list.track_pin(list.pin_at_row(3, 1).unwrap()).unwrap();
    list.erase_active(0);
    assert_eq!(letters(&list), vec!['A', 'B', 'D', 'E', '\0']);
    assert_eq!(list.tracked_pin(before).unwrap().y, 1);
    assert_eq!(
        (
            list.tracked_pin(removed).unwrap().x,
            list.tracked_pin(removed).unwrap().y
        ),
        (0, 2)
    );
    assert_eq!(list.tracked_pin(after).unwrap().y, 2);
}

#[test]
fn history_erase_row_bounded_with_pin_at_top() {
    bounded_single(0, 3, &[(5, 0, 0, 0)]);
    let mut list = letter_rows("ABCDE", 3);
    let handle = list.track_pin(list.pin_at_row(0, 2).unwrap()).unwrap();
    list.erase_row_bounded(Point::Screen(Coordinate::default()), 2)
        .unwrap();
    assert_eq!(letters(&list), vec!['B', 'C', '\0', 'D', 'E']);
    assert_eq!(
        (
            list.tracked_pin(handle).unwrap().x,
            list.tracked_pin(handle).unwrap().y
        ),
        (0, 0)
    );
}

#[test]
fn history_erase_row_bounded_full_rows_single_page() {
    bounded_single(5, 10, &[(0, 7, 0, 6), (0, 9, 0, 8)]);
    let mut list = letter_rows("ABCDE", 3);
    let handle = list.track_pin(list.pin_at_row(4, 1).unwrap()).unwrap();
    list.erase_row_bounded(Point::Screen(Coordinate { x: 0, y: 2 }), usize::MAX)
        .unwrap();
    assert_eq!(letters(&list), vec!['A', 'B', 'D', 'E', '\0']);
    assert_eq!(list.tracked_pin(handle).unwrap().y, 3);
}

#[test]
fn history_erase_row_bounded_full_rows_two_pages() {
    bounded_two_pages();
    let mut list = letter_rows("ABCDE", 3);
    list.split(list.pin_at_row(2, 0).unwrap()).unwrap();
    let handle = list.track_pin(list.pin_at_row(2, 1).unwrap()).unwrap();
    list.erase_row_bounded(Point::Screen(Coordinate { x: 0, y: 1 }), 2)
        .unwrap();
    assert_eq!(letters(&list), vec!['A', 'C', 'D', '\0', 'E']);
    assert_eq!(
        (
            list.tracked_pin(handle).unwrap().node,
            list.tracked_pin(handle).unwrap().y
        ),
        (list.first, 1)
    );
}

#[test]
fn history_erase_a_one_row_active() {
    let mut source = screen(10, 1);
    assert_eq!(source.total_pages(), 1);
    super::resize::write(&mut source, 0, "A");
    source.erase_active(0);
    assert_eq!(source.total_rows(), 1);
    assert_eq!(
        source
            .get_cell(Point::Active(Coordinate::default()))
            .unwrap()
            .cell
            .codepoint(),
        '\0'
    );
    integrity(&source);
    let mut list = letter_rows("A", 1);
    let node = list.first;
    list.erase_active(0);
    assert_eq!(letters(&list), vec!['\0']);
    assert!(list.node_is_valid(node));
    assert_eq!(list.total_pages(), 1);
}

#[test]
fn history_erase_row_with_tracked_pin_shifts() {
    let mut source = screen(80, 24);
    let pin = source
        .track_pin(
            source
                .pin(Point::Active(Coordinate { x: 2, y: 4 }))
                .unwrap(),
        )
        .unwrap();
    source.erase_active(3);
    assert_eq!(source.total_rows(), 24);
    locations(&source, &[(pin, source.first, 2, 0)]);
    integrity(&source);
    let mut list = letter_rows("ABCDE", 5);
    let pin = list.track_pin(list.pin_at_row(4, 2).unwrap()).unwrap();
    list.erase_active(3);
    assert_eq!(letters(&list), vec!['E', '\0', '\0', '\0', '\0']);
    assert_eq!(
        (
            list.tracked_pin(pin).unwrap().x,
            list.tracked_pin(pin).unwrap().y
        ),
        (2, 0)
    );
}

#[test]
fn history_erase_row_with_tracked_pin_is_erased() {
    let mut source = screen(80, 24);
    let pin = source
        .track_pin(
            source
                .pin(Point::Active(Coordinate { x: 2, y: 2 }))
                .unwrap(),
        )
        .unwrap();
    source.erase_active(3);
    assert_eq!(source.total_rows(), 24);
    locations(&source, &[(pin, source.first, 0, 0)]);
    integrity(&source);
    let mut list = letter_rows("ABCDE", 5);
    let pin = list.track_pin(list.pin_at_row(2, 2).unwrap()).unwrap();
    list.erase_active(3);
    assert_eq!(
        (
            list.tracked_pin(pin).unwrap().x,
            list.tracked_pin(pin).unwrap().y
        ),
        (0, 0)
    );
    assert!(!list.tracked_pin(pin).unwrap().garbage);
}

#[test]
fn history_erase_row_with_tracked_pin_resets_to_top_left() {
    let mut source = bulk_history();
    let pin = source
        .track_pin(source.pin(Point::History(Coordinate::default())).unwrap())
        .unwrap();
    source.erase_history(None);
    assert_eq!(source.total_rows(), 24);
    locations(&source, &[(pin, source.first, 0, 0)]);
    integrity(&source);
    let mut list = letter_rows("ABCDE", 2);
    list.split(list.pin_at_row(2, 0).unwrap()).unwrap();
    let old = list.first;
    let handle = list.track_pin(list.pin_at_row(1, 2).unwrap()).unwrap();
    list.erase_history(None);
    assert_eq!(letters(&list), vec!['D', 'E']);
    assert_eq!(
        (
            list.tracked_pin(handle).unwrap().x,
            list.tracked_pin(handle).unwrap().y
        ),
        (0, 0)
    );
    assert!(!list.node_is_valid(old));
}

#[test]
fn history_erase() {
    let mut source = bulk_history();
    source.erase_history(None);
    assert_eq!((source.total_rows(), source.total_pages()), (24, 1));
    assert_eq!(source.first, source.last);
    integrity(&source);
    let mut list = letter_rows("ABCDE", 2);
    list.split(list.pin_at_row(2, 0).unwrap()).unwrap();
    list.split(list.pin_at_row(3, 0).unwrap()).unwrap();
    list.erase_history(None);
    assert_eq!(letters(&list), vec!['D', 'E']);
    assert_eq!((list.total_rows(), list.total_pages()), (2, 1));
}

#[test]
fn history_erase_reaccounts_page_size() {
    let mut source = bulk_history();
    let original = PageList::page_charge(PageList::initial_capacity(80));
    assert!(source.charged_bytes() > original);
    source.erase_history(None);
    assert_eq!(source.charged_bytes(), original);
    integrity(&source);
    let mut list = letter_rows("ABCDE", 2);
    let original = list.charged_bytes();
    list.split(list.pin_at_row(3, 0).unwrap()).unwrap();
    assert!(list.charged_bytes() > original);
    list.erase_history(None);
    assert_eq!(list.charged_bytes(), original);
}

#[test]
fn history_erase_resets_viewport_to_active_if_moves_within_active() {
    let mut source = bulk_history();
    source.scroll(Scroll::DeltaRow(
        -isize::try_from(source.total_rows()).unwrap(),
    ));
    assert_eq!(source.viewport, Viewport::Top);
    source.erase_history(None);
    assert_eq!(source.viewport, Viewport::Active);
    integrity(&source);
    let mut list = letter_rows("ABCDE", 2);
    list.scroll(Scroll::Row(2));
    list.erase_history(None);
    assert_eq!(list.viewport, Viewport::Active);
    assert_eq!(list.scrollbar().offset, 0);
}

#[test]
fn history_erase_resets_viewport_if_inside_erased_page_but_not_active() {
    let mut source = bulk_history();
    source.scroll(Scroll::DeltaRow(
        -isize::try_from(source.total_rows()).unwrap(),
    ));
    assert_eq!(source.viewport, Viewport::Top);
    source.erase_history(Some(Point::History(Coordinate { x: 0, y: 2 })));
    assert_eq!(source.viewport, Viewport::Top);
    integrity(&source);
    let mut list = letter_rows("ABCDE", 2);
    list.scroll(Scroll::Top);
    list.erase_history(Some(Point::History(Coordinate::default())));
    assert_eq!(list.viewport, Viewport::Top);
    assert_eq!(letters(&list), vec!['B', 'C', 'D', 'E']);
}

#[test]
fn history_erase_resets_viewport_to_active_if_top_is_inside_active() {
    let mut source = bulk_history();
    source.scroll(Scroll::Top);
    source.erase_history(None);
    assert_eq!(source.viewport, Viewport::Active);
    integrity(&source);
    let mut list = letter_rows("ABCDE", 2);
    list.scroll(Scroll::Top);
    list.erase_history(None);
    assert_eq!(list.viewport, Viewport::Active);
}

#[test]
fn erase_row_crosses_pages_and_clears_only_the_last_row() {
    let mut list = letter_rows("ABCDE", 3);
    list.split(list.pin_at_row(2, 0).unwrap()).unwrap();
    let first = list.first;
    let second = list.last;
    let tracked = list.track_pin(list.pin_at_row(2, 2).unwrap()).unwrap();
    list.erase_row(Point::Screen(Coordinate { x: 0, y: 1 }))
        .unwrap();
    assert_eq!(letters(&list), vec!['A', 'C', 'D', 'E', '\0']);
    assert_eq!(
        (
            list.tracked_pin(tracked).unwrap().node,
            list.tracked_pin(tracked).unwrap().y
        ),
        (first, 1)
    );
    assert!(list.node_is_valid(first) && list.node_is_valid(second));
    assert_eq!(list.total_rows(), 5);
}

#[test]
fn erase_row_boundary_capacity_failure_keeps_managed_ownership_valid() {
    use crate::{
        page::StorageError,
        style::{Color, Style},
    };
    let mut list = letter_rows("ABCDE", 3);
    list.split(list.pin_at_row(2, 0).unwrap()).unwrap();
    let mut capacity = list.page(list.first).unwrap().capacity;
    capacity.styles = 0;
    let mut replacement = Page::init(capacity);
    replacement.size.rows = 2;
    list.nodes.get_mut(list.first).unwrap().page = replacement;
    let source = list.page_mut(list.last).unwrap();
    let id = source
        .styles
        .add(Style {
            fg_color: Color::Palette(4),
            ..Style::default()
        })
        .unwrap();
    source.get_cells_mut(0)[0].set_style_id(id);
    source.get_row_mut(0).set_styled(true);
    let handle = list.track_pin(list.pin_at_row(2, 0).unwrap()).unwrap();
    assert_eq!(
        list.erase_row(Point::Screen(Coordinate::default())),
        Err(StorageError::StyleSetOutOfMemory)
    );
    assert_eq!(list.total_rows(), 5);
    assert_eq!(list.nodes.get(list.first).unwrap().next, Some(list.last));
    assert_eq!(list.nodes.get(list.last).unwrap().prev, Some(list.first));
    assert!(list.pin_is_valid(*list.tracked_pin(handle).unwrap()));
    assert_eq!(list.page(list.last).unwrap().get_cells(0)[0].style_id(), id);
    assert_eq!(list.page(list.first).unwrap().get_cells(1)[0].style_id(), 0);
    list.page_mut(list.first).unwrap().clear_cells(1, 0..3);
    list.page_mut(list.last).unwrap().clear_cells(0, 0..3);
    assert_eq!(list.page(list.last).unwrap().styles.count(), 0);
}

/// Retain several complete pages and cache a historical viewport position.
fn cached_history(viewport_row: usize, pages: usize) -> PageList {
    let mut list = PageList::init(
        NonZeroU16::new(80).unwrap(),
        NonZeroU16::new(24).unwrap(),
        None,
    );
    let capacity = usize::from(list.page(list.first).unwrap().capacity.rows);
    for _ in 0..capacity * pages {
        list.grow();
    }
    list.scroll(Scroll::Row(viewport_row));
    cache_view(&mut list, viewport_row);
    list
}

/// Check the complete cached viewport observation, not only its offset.
fn cache_view(list: &mut PageList, offset: usize) {
    assert_eq!(list.viewport, Viewport::Pin);
    assert_eq!(
        list.scrollbar(),
        Scrollbar {
            total: list.total_rows(),
            offset,
            len: usize::from(list.rows())
        }
    );
}

#[test]
fn history_erase_rows_invalidates_viewport_offset_cache() {
    let capacity = usize::from(PageList::initial_capacity(80).rows);
    for (pin_y, erased) in [(30, 10), (capacity, capacity / 2)] {
        let mut list = cached_history(pin_y, 3);
        list.erase_history(Some(Point::History(Coordinate {
            x: 0,
            y: u32::try_from(erased - 1).unwrap(),
        })));
        cache_view(&mut list, pin_y - erased);
        integrity(&list);
    }
}

#[test]
fn history_erase_row_invalidates_viewport_offset_cache() {
    let capacity = usize::from(PageList::initial_capacity(80).rows);
    for pin_y in [30, capacity] {
        let mut list = cached_history(pin_y, 3);
        list.erase_row(Point::History(Coordinate::default()))
            .unwrap();
        cache_view(&mut list, pin_y - 1);
        integrity(&list);
    }
}

#[test]
fn history_erase_row_bounded_invalidates_viewport_offset_cache() {
    let mut list = cached_history(4, 3);
    list.erase_row_bounded(Point::History(Coordinate::default()), 10)
        .unwrap();
    cache_view(&mut list, 3);
}

#[test]
fn history_erase_row_bounded_multi_page_invalidates_viewport_offset_cache() {
    let mut list = cached_history(1, 3);
    let capacity = usize::from(list.page(list.first).unwrap().capacity.rows);
    list.scroll(Scroll::Row(capacity + 1));
    cache_view(&mut list, capacity + 1);
    list.erase_row_bounded(Point::History(Coordinate::default()), capacity + 10)
        .unwrap();
    cache_view(&mut list, capacity);
}

#[test]
fn history_erase_row_bounded_full_page_shift_invalidates_viewport_offset_cache() {
    let mut list = cached_history(5, 4);
    let capacity = usize::from(list.page(list.first).unwrap().capacity.rows);
    list.erase_row_bounded(Point::History(Coordinate::default()), capacity * 2 + 10)
        .unwrap();
    cache_view(&mut list, 4);
}

#[test]
fn history_erase_row_bounded_exhausts_pages_invalidates_viewport_offset_cache() {
    let mut list = cached_history(1, 3);
    let capacity = usize::from(list.page(list.first).unwrap().capacity.rows);
    list.scroll(Scroll::Row(capacity * 2 + 10));
    let row = list.scrollbar().offset;
    let total = list.total_rows();
    list.erase_row_bounded(Point::History(Coordinate::default()), total * 2)
        .unwrap();
    cache_view(&mut list, row - 1);
    assert_eq!(list.total_rows(), total);
}

#[test]
fn erased_row_rotation_preserves_managed_cell_ownership() {
    use crate::page::{CellPosition, cell::Content};
    let mut list = letter_rows("ABCDE", 3);
    let position = CellPosition { x: 1, y: 2 };
    let origin = list.pin_at_row(0, 1).unwrap();
    populate_pin_views(&mut list, origin, crate::Style::default());
    let page = list.page_mut(list.first).unwrap();
    page.get_cells_mut(position.y)[usize::from(position.x)].set_content(Content::Codepoint('z'));
    page.append_grapheme(position, '\u{302}').unwrap();
    let before = page.lookup_grapheme(position).unwrap().to_vec();
    list.erase_row_bounded(Point::Screen(Coordinate { x: 0, y: 1 }), 1)
        .unwrap();
    let page = list.page(list.first).unwrap();
    assert_eq!(
        page.lookup_grapheme(CellPosition { x: 1, y: 1 }),
        Some(before.as_slice())
    );
    assert_eq!(page.lookup_grapheme(position), None);
    let cloned = page.clone();
    assert_eq!(
        cloned.lookup_grapheme(CellPosition { x: 1, y: 1 }),
        Some(before.as_slice())
    );
    list.erase_history(Some(Point::History(Coordinate { x: 0, y: 1 })));
    let page = list.page(list.first).unwrap();
    assert_eq!(page.lookup_grapheme(CellPosition { x: 1, y: 0 }), None);
}

#[test]
fn history_erasure_keeps_live_node_identity() {
    let mut list = letter_rows("ABC", 3);
    let captured = list.pin_at_row(1, 0).unwrap();
    let tracked = list.track_pin(captured).unwrap();
    list.erase_active(0);
    assert!(list.node_is_valid(captured.node));
    assert_eq!(captured.row_and_cell(&list).unwrap().1.codepoint(), 'C');
    let retained = *list.tracked_pin(tracked).unwrap();
    assert_eq!(retained.row_and_cell(&list).unwrap().1.codepoint(), 'B');
    assert_eq!(retained.y, 0);
}

#[test]
fn erase_boundary_copy_retains_clusters_styles_and_links() {
    use crate::{
        page::CellPosition,
        style::{Color, Style},
    };
    let mut list = letter_rows("ABCDE", 3);
    list.split(list.pin_at_row(2, 0).unwrap()).unwrap();
    let pin = list.pin_at_row(2, 1).unwrap();
    let style = Style {
        fg_color: Color::Palette(7),
        ..Style::default()
    };
    populate_pin_views(&mut list, pin, style);
    let tracked = list.track_pin(pin).unwrap();
    list.erase_row(Point::Screen(Coordinate { x: 0, y: 1 }))
        .unwrap();
    let moved = *list.tracked_pin(tracked).unwrap();
    assert_eq!(moved.node, list.first);
    assert_eq!(
        moved.grapheme(&list, CellPosition { x: 1, y: 1 }),
        Some(&['\u{301}'][..])
    );
    assert_eq!(moved.hyperlink_uri(&list), Some(&[b'x', 0xff, 0][..]));
    assert_eq!(
        moved.style(&list, *moved.row_and_cell(&list).unwrap().1),
        Some(style)
    );
    let source = list.page(list.last).unwrap();
    assert_eq!(source.lookup_grapheme(CellPosition { x: 1, y: 2 }), None);
    list.erase_active(0);
    assert!(list.pin_is_valid(*list.tracked_pin(tracked).unwrap()));
}

#[test]
fn erase_active_across_pages_preserves_history_prefix_and_regrows() {
    let mut list = letter_rows("ABCDEFGHI", 6);
    list.split(list.pin_at_row(4, 0).unwrap()).unwrap();
    list.split(list.pin_at_row(7, 0).unwrap()).unwrap();
    let first = list.first;
    let middle = list.nodes.get(first).unwrap().next.unwrap();
    let final_node = list.last;
    let kept = list.track_pin(list.pin_at_row(8, 1).unwrap()).unwrap();
    list.erase_active(4);
    assert_eq!(
        letters(&list),
        vec!['A', 'B', 'C', 'I', '\0', '\0', '\0', '\0', '\0']
    );
    assert!(list.node_is_valid(first) && list.node_is_valid(final_node));
    assert!(!list.node_is_valid(middle));
    assert_eq!(list.nodes.get(first).unwrap().next, Some(final_node));
    assert_eq!(list.nodes.get(final_node).unwrap().prev, Some(first));
    assert_eq!(list.tracked_pin(kept).unwrap().y, 0);
    assert_eq!((list.total_rows(), list.history_rows()), (9, 3));
}

#[test]
fn erase_bounded_zero_clears_only_the_target_row() {
    let mut list = letter_rows("ABCDE", 3);
    let handle = list.track_pin(list.pin_at_row(2, 1).unwrap()).unwrap();
    list.erase_row_bounded(Point::Screen(Coordinate { x: 0, y: 2 }), 0)
        .unwrap();
    assert_eq!(letters(&list), vec!['A', 'B', '\0', 'D', 'E']);
    assert_eq!(list.tracked_pin(handle).unwrap().y, 1);
}
