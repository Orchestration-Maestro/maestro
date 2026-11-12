//! Allocation and bounded pruning keep the active area and retained pins valid.

use super::resize::{integrity, nz, screen, screen_origin};
use super::*;
use crate::point::Tag;

/// A bounded standard-width screen.
fn bounded(pages: u64) -> PageList {
    PageList::init(
        nz(80),
        nz(24),
        Some(usize::try_from(pages * PageList::standard_bytes()).unwrap()),
    )
}

/// Reach the next allocation boundary without appending another page.
fn fill_last(list: &mut PageList) {
    while list.page(list.last).unwrap().size.rows < list.page(list.last).unwrap().capacity.rows {
        assert!(list.grow().is_none());
    }
}

/// Allocate the requested initial pages before a pruning cycle begins.
fn pages(list: &mut PageList, count: usize) {
    while list.total_pages() < count {
        list.grow();
    }
}

/// Force one bounded pruning cycle and check every surviving node and link.
fn prune(list: &mut PageList) {
    let first = list.first;
    fill_last(list);
    assert!(list.grow().is_some());
    assert!(!list.node_is_valid(first));
    integrity(list);
    let mut current = Some(list.first);
    while let Some(key) = current {
        assert!(list.node_is_valid(key));
        assert!(list.pin_is_valid(Pin {
            node: key,
            x: 0,
            y: 0,
            garbage: false
        }));
        current = list.nodes.get(key).unwrap().next;
    }
}

#[test]
fn history_grow_fit_in_capacity() {
    let mut list = screen(80, 24);
    assert!(list.page(list.last).unwrap().size.rows < list.page(list.last).unwrap().capacity.rows);
    assert!(list.grow().is_none());
    screen_origin(&list, Tag::Active, 1);
    assert_eq!(
        list.point_from_pin(Tag::Screen, list.get_top_left(Tag::Active)),
        Some(Point::Screen(Coordinate { x: 0, y: 1 }))
    );
}

#[test]
fn history_grow_allows_exceeding_max_size_for_active_area() {
    let rows = PageList::initial_capacity(5).rows;
    let mut list = PageList::init(nz(5), nz(rows), Some(0));
    assert_eq!(list.total_rows(), usize::from(rows));
    list.grow();
    let count = list.total_pages();
    assert!(count >= 2);
    for node in list.nodes.values_mut() {
        node.page.size.rows = 1;
        node.page.capacity.rows = 1;
    }
    list.total_rows = count;
    list.grow();
    assert_eq!(list.total_pages(), count + 1);
}

/// Increase suffix storage until the first page is larger than a standard page.
fn nonstandard(list: &mut PageList) {
    while PageList::page_charge(list.page(list.first).unwrap().capacity)
        <= PageList::standard_bytes()
    {
        list.increase_capacity(list.first, Some(IncreaseCapacity::GraphemeBytes))
            .unwrap();
    }
}

#[test]
fn history_grow_prune_required_with_a_single_page() {
    let mut list = screen(80, 24);
    nonstandard(&mut list);
    assert_eq!(list.first, list.last);
    fill_last(&mut list);
    let new = list.grow().unwrap();
    assert_ne!(new, list.first);
    let total = list.total_rows();
    assert_eq!(
        list.scrollbar(),
        Scrollbar {
            total,
            offset: total - 24,
            len: 24
        }
    );
    integrity(&list);
}

#[test]
fn history_grow_prune_scrollback() {
    let mut list = bounded(1);
    pages(&mut list, 2);
    fill_last(&mut list);
    let (first, second, charge) = (list.first, list.last, list.charged_bytes());
    let rows = list.page(first).unwrap().size.rows;
    let handle = list.track_pin(list.get_top_left(Tag::Screen)).unwrap();
    list.scroll(Scroll::Row(usize::from(rows / 2)));
    assert_eq!(list.viewport, Viewport::Pin);
    assert_eq!(list.scrollbar().offset, usize::from(rows / 2));
    let new = list.grow().unwrap();
    assert_eq!((list.first, list.last), (second, new));
    assert!(!list.node_is_valid(first));
    assert_eq!(list.charged_bytes(), charge);
    assert_eq!(
        *list.tracked_pin(handle).unwrap(),
        Pin {
            node: second,
            x: 0,
            y: 0,
            garbage: true
        }
    );
    assert!(list.tracked_pin(handle).unwrap().garbage);
    assert_eq!(list.scrollbar().offset, 0);
    integrity(&list);
}

#[test]
fn history_grow_prune_scrollback_with_viewport_pin_not_in_pruned_page() {
    let mut list = bounded(1);
    pages(&mut list, 2);
    fill_last(&mut list);
    let (first, second, charge) = (list.first, list.last, list.charged_bytes());
    let rows = list.page(first).unwrap().size.rows;
    list.scroll(Scroll::Row(usize::from(rows) + 5));
    assert_eq!(list.viewport, Viewport::Pin);
    assert_eq!(list.tracked_pin(list.viewport_pin).unwrap().node, second);
    assert_eq!(list.scrollbar().offset, usize::from(rows) + 5);
    let pin = *list.tracked_pin(list.viewport_pin).unwrap();
    list.grow();
    assert_eq!(list.first, second);
    assert_eq!(list.charged_bytes(), charge);
    assert_eq!(list.tracked_pin(list.viewport_pin), Some(&pin));
    assert_eq!(list.scrollbar().offset, 5);
    integrity(&list);
}

#[test]
fn history_grow_reuses_non_standard_page_without_leak() {
    let mut list = bounded(3);
    nonstandard(&mut list);
    fill_last(&mut list);
    list.grow();
    assert_ne!(list.first, list.last);
    while list.charged_bytes() + PageList::standard_bytes() <= list.max_size()
        || list.page(list.last).unwrap().size.rows < list.page(list.last).unwrap().capacity.rows
    {
        list.grow();
    }
    let first = list.first;
    let charge = PageList::page_charge(list.page(first).unwrap().capacity);
    assert!(charge > PageList::standard_bytes());
    assert!(list.total_rows() >= usize::from(list.rows()));
    let handle = list.track_pin(list.get_top_left(Tag::Screen)).unwrap();
    let old_charge = list.charged_bytes();
    assert_eq!(
        list.page(list.last).unwrap().size.rows,
        list.page(list.last).unwrap().capacity.rows
    );
    list.grow();
    assert_ne!(list.last, first);
    assert_ne!(list.first, first);
    assert!(!list.node_is_valid(first));
    assert_eq!(
        list.charged_bytes(),
        old_charge - charge + PageList::standard_bytes()
    );
    let pin = *list.tracked_pin(handle).unwrap();
    assert_eq!(
        (pin.node, pin.x, pin.y, pin.garbage),
        (list.first, 0, 0, true)
    );
    integrity(&list);
}

#[test]
fn history_grow_non_standard_page_prune_protection() {
    let mut list = PageList::init(
        nz(80),
        nz(600),
        Some(usize::try_from(PageList::standard_bytes()).unwrap()),
    );
    nonstandard(&mut list);
    let first = list.first;
    assert!(PageList::page_charge(list.page(first).unwrap().capacity) > PageList::standard_bytes());
    let rows = list.page(first).unwrap().size.rows;
    pages(&mut list, 2);
    assert_ne!(list.first, list.last);
    fill_last(&mut list);
    assert!(list.total_rows() - usize::from(rows) + 1 < usize::from(list.rows()));
    assert!(list.charged_bytes() + PageList::standard_bytes() > list.max_size());
    assert!(list.total_rows() >= usize::from(list.rows()));
    list.grow();
    assert_eq!(list.first, first);
    assert!(list.total_rows() >= usize::from(list.rows()));
    integrity(&list);
}

#[test]
fn history_bounded_pruning_after_partial_erase_preserves_live_nodes() {
    let mut list = bounded(2);
    pages(&mut list, 2);
    let first = list.first;
    let rows = list.page(first).unwrap().size.rows;
    list.erase_history(Some(Point::History(Coordinate::default())));
    assert_eq!(list.first, first);
    assert_eq!(list.page(first).unwrap().size.rows, rows - 1);
    assert!(list.node_is_valid(first));
    prune(&mut list);
}

#[test]
fn history_bounded_pruning_after_split_invalidation_preserves_live_nodes() {
    let mut list = bounded(2);
    pages(&mut list, 2);
    let first = list.first;
    let pin = Pin {
        node: first,
        x: 0,
        y: list.page(first).unwrap().size.rows / 2,
        garbage: false,
    };
    list.split(pin).unwrap();
    assert!(list.node_is_valid(first));
    assert!(!list.pin_is_valid(pin));
    prune(&mut list);
}

#[test]
fn history_repeated_bounded_pruning_after_split_preserves_live_nodes() {
    let mut list = bounded(3);
    pages(&mut list, 3);
    let first = list.first;
    list.split(Pin {
        node: first,
        x: 0,
        y: list.page(first).unwrap().size.rows / 2,
        garbage: false,
    })
    .unwrap();
    let split = list.nodes.get(first).unwrap().next.unwrap();
    prune(&mut list);
    assert_eq!(list.first, split);
    prune(&mut list);
    assert!(!list.node_is_valid(split));
}

#[test]
fn history_bounded_pruning_after_front_replacement_preserves_live_nodes() {
    let mut list = bounded(2);
    pages(&mut list, 2);
    let first = list.first;
    let replacement = list.increase_capacity(first, None).unwrap();
    assert_ne!(first, replacement);
    assert!(!list.node_is_valid(first));
    prune(&mut list);
    assert!(!list.node_is_valid(replacement));
}

#[test]
fn history_bounded_pruning_after_middle_replacement_preserves_live_nodes() {
    let mut list = bounded(3);
    pages(&mut list, 3);
    let middle = list.nodes.get(list.first).unwrap().next.unwrap();
    let replacement = list.increase_capacity(middle, None).unwrap();
    assert_ne!(middle, replacement);
    assert!(!list.node_is_valid(middle));
    prune(&mut list);
    assert_eq!(list.first, replacement);
    prune(&mut list);
    assert!(!list.node_is_valid(replacement));
}

#[test]
fn history_trimming_keeps_live_nodes_and_checks_bounds() {
    let mut list = screen(80, 24);
    let first = list.first;
    let retained = list.get_top_left(Tag::Active);
    let trimmed = list.get_bottom_right(Tag::Active).unwrap();
    list.resize(Resize {
        rows: Some(nz(23)),
        reflow: false,
        ..Resize::default()
    })
    .unwrap();
    assert_eq!(list.total_rows(), 23);
    assert_eq!(list.first, first);
    assert!(list.node_is_valid(first));
    assert!(list.pin_is_valid(retained));
    assert!(retained.row_and_cell(&list).is_some());
    assert!(!list.pin_is_valid(trimmed));
    assert!(trimmed.row_and_cell(&list).is_none());
    list.grow();
    assert!(list.pin_is_valid(trimmed));
    integrity(&list);
}
