//! Inclusive traversal limits across active and historical page boundaries.

use super::resize::{nz, screen, write};
use super::*;

/// A full page followed by one row, with the active region spanning both.
fn two_pages() -> PageList {
    let mut list = screen(80, 24);
    let rows = usize::from(list.page(list.first).unwrap().capacity.rows);
    for _ in list.total_rows()..rows {
        assert!(list.grow().is_none());
    }
    assert!(list.grow().is_some());
    list
}

/// Assert one chunk's identity and half-open bounds.
fn chunk(it: &mut PageIterator, list: &PageList, node: NodeId, start: u16, end: u16) -> u16 {
    let actual = it.next(list).unwrap();
    assert_eq!(actual, Chunk { node, start, end });
    actual.end - actual.start
}

#[test]
fn history_page_iterator_two_pages() {
    let list = two_pages();
    let rows = list.page(list.first).unwrap().size.rows;
    let mut it = list.page_iterator(
        Direction::RightDown,
        Point::Active(Coordinate::default()),
        None,
    );
    chunk(&mut it, &list, list.first, rows - 23, rows);
    chunk(&mut it, &list, list.last, 0, 1);
    assert!(it.next(&list).is_none());
}

#[test]
fn history_page_iterator_history_two_pages() {
    let list = two_pages();
    let mut it = list.page_iterator(
        Direction::RightDown,
        Point::History(Coordinate::default()),
        None,
    );
    chunk(
        &mut it,
        &list,
        list.first,
        0,
        list.get_top_left(crate::point::Tag::Active).y,
    );
    assert!(it.next(&list).is_none());
}

#[test]
fn history_page_iterator_reverse_single_page() {
    let list = screen(80, 24);
    assert_eq!(list.first, list.last);
    let mut it = list.page_iterator(
        Direction::LeftUp,
        Point::Active(Coordinate::default()),
        None,
    );
    chunk(&mut it, &list, list.first, 0, 24);
    assert!(it.next(&list).is_none());
}

#[test]
fn history_page_iterator_reverse_two_pages() {
    let list = two_pages();
    let rows = list.page(list.first).unwrap().size.rows;
    let mut it = list.page_iterator(
        Direction::LeftUp,
        Point::Active(Coordinate::default()),
        None,
    );
    let count =
        chunk(&mut it, &list, list.last, 0, 1) + chunk(&mut it, &list, list.first, rows - 23, rows);
    assert!(it.next(&list).is_none());
    assert_eq!(count, list.rows());
}

#[test]
fn history_page_iterator_reverse_history_two_pages() {
    let list = two_pages();
    let mut it = list.page_iterator(
        Direction::LeftUp,
        Point::History(Coordinate::default()),
        None,
    );
    chunk(
        &mut it,
        &list,
        list.first,
        0,
        list.get_top_left(crate::point::Tag::Active).y,
    );
    assert!(it.next(&list).is_none());
}

#[test]
fn history_page_iterator_reverse_count_includes_row_zero() {
    let list = screen(2, 2);
    let pin = list.get_top_left(crate::point::Tag::Screen);
    let mut it = pin.page_iterator(Direction::LeftUp, Some(pin));
    chunk(&mut it, &list, list.first, 0, 1);
    assert!(it.next(&list).is_none());
}

#[test]
fn history_page_iterator_count_crosses_page_boundaries() {
    let list = two_pages();
    let rows = list.page(list.first).unwrap().size.rows;
    let first = list.pin_at_row(usize::from(rows) - 1, 0).unwrap();
    let second = list.pin_at_row(usize::from(rows), 0).unwrap();
    let mut down = first.page_iterator(Direction::RightDown, Some(second));
    chunk(&mut down, &list, list.first, rows - 1, rows);
    chunk(&mut down, &list, list.last, 0, 1);
    assert!(down.next(&list).is_none());
    let mut up = second.page_iterator(Direction::LeftUp, Some(first));
    chunk(&mut up, &list, list.last, 0, 1);
    chunk(&mut up, &list, list.first, rows - 1, rows);
    assert!(up.next(&list).is_none());
}

#[test]
fn history_cell_iterator_reverse() {
    let mut list = PageList::init(nz(2), nz(2), Some(0));
    assert_eq!(list.first, list.last);
    for y in 0..2 {
        write(&mut list, y, "\0\u{1}");
    }
    let mut it = list.cell_iterator(
        Direction::LeftUp,
        Point::Screen(Coordinate::default()),
        None,
    );
    for (x, y) in [(1, 1), (0, 1), (1, 0), (0, 0)] {
        let pin = it.next(&list).unwrap();
        assert_eq!(
            list.point_from_pin(crate::point::Tag::Screen, pin),
            Some(Point::Screen(Coordinate { x, y }))
        );
        assert_eq!(
            pin.row_and_cell(&list).unwrap().1.codepoint(),
            if x == 0 { '\0' } else { '\u{1}' }
        );
    }
    assert!(it.next(&list).is_none());
}

#[test]
fn iteration_work_is_monotone() {
    let small = iteration_work(64);
    let large = iteration_work(128);
    for (before, after) in small[..3].iter().zip(&large[..3]) {
        assert!(after > before);
        assert!(*after <= before * 2);
    }
    // The single-row control has one shared final boundary check, not one per row.
    assert_eq!(large[3] + 1, (small[3] + 1) * 2);
}

/// Measure real adapter lookups as well as yielded chunks, rows and cells.
fn iteration_work(rows: u16) -> [usize; 4] {
    let mut list = screen(5, rows);
    for y in (16..rows).step_by(16) {
        let pin = list.pin_at_row(usize::from(y), 0).unwrap();
        list.split(pin).unwrap();
    }
    let origin = list.get_top_left(crate::point::Tag::Screen);
    crate::tests::WORK.with_borrow_mut(|work| work.queries.clear());
    let mut pages = list.page_iterator(
        Direction::RightDown,
        Point::Screen(Coordinate::default()),
        None,
    );
    let (mut chunks, mut row_count, mut cell_count) = (0, 0, 0);
    while let Some(chunk) = pages.next(&list) {
        chunks += 1;
        let page = list.page(chunk.node).unwrap();
        for y in chunk.start..chunk.end {
            row_count += 1;
            cell_count += page.get_cells(y).len();
        }
    }
    let indexed = crate::tests::WORK
        .with_borrow(|work| work.queries.get(&list.first.arena).copied().unwrap_or(0));
    let rows = usize::from(rows);
    assert_eq!((chunks, row_count, cell_count), (rows / 16, rows, rows * 5));
    crate::tests::WORK.with_borrow_mut(|work| work.queries.clear());
    let mut row_it = list.row_iterator(
        Direction::RightDown,
        Point::Screen(Coordinate::default()),
        None,
    );
    for row in 0..rows {
        let pin = row_it.next(&list).unwrap();
        assert_eq!((usize::from(pin.y), pin.x), (row % 16, 0));
    }
    assert!(row_it.next(&list).is_none());
    let row_queries = crate::tests::WORK
        .with_borrow(|work| work.queries.get(&list.first.arena).copied().unwrap_or(0));
    crate::tests::WORK.with_borrow_mut(|work| work.queries.clear());
    let mut cells = list.cell_iterator(
        Direction::RightDown,
        Point::Screen(Coordinate::default()),
        None,
    );
    for cell in 0..rows * 5 {
        let pin = cells.next(&list).unwrap();
        assert_eq!(
            (usize::from(pin.y), usize::from(pin.x)),
            ((cell / 5) % 16, cell % 5)
        );
    }
    assert!(cells.next(&list).is_none());
    let cell_queries = crate::tests::WORK
        .with_borrow(|work| work.queries.get(&list.first.arena).copied().unwrap_or(0));
    let stateful = iteration_controls(&list, origin, rows);
    assert!(indexed < row_queries && row_queries < cell_queries);
    eprintln!(
        "WORK rows={rows} indexed={indexed} row={row_queries} cell={cell_queries} stateful={stateful}"
    );
    [indexed, row_queries, cell_queries, stateful]
}

/// Compare stateful single-row advancement with actual repeated prefix walks.
fn iteration_controls(list: &PageList, origin: Pin, rows: usize) -> usize {
    crate::tests::WORK.with_borrow_mut(|work| work.queries.clear());
    let mut pin = origin;
    for _ in 1..rows {
        pin = pin.down(list, 1).unwrap();
    }
    assert!(pin.down(list, 1).is_none());
    let stateful = crate::tests::WORK
        .with_borrow(|work| work.queries.get(&list.first.arena).copied().unwrap_or(0));
    crate::tests::WORK.with_borrow_mut(|work| work.queries.clear());
    let mut repeated = 0;
    for target in 0..rows {
        let mut pin = origin;
        for _ in 0..target {
            pin = pin.down(list, 1).unwrap();
            repeated += 1;
        }
        assert_eq!(usize::from(pin.y), target % 16);
    }
    assert_eq!(repeated, rows * (rows - 1) / 2);
    assert_eq!(repeated, if rows == 64 { 2016 } else { 8128 });
    assert!(
        crate::tests::WORK.with_borrow(|work| work
            .queries
            .get(&list.first.arena)
            .copied()
            .unwrap_or(0))
            > stateful * 10
    );
    stateful
}

#[test]
fn pin_row_and_cell_iterators_include_row_limits() {
    let mut list = screen(2, 3);
    let first = list.pin_at_row(0, 1).unwrap();
    let bottom = list.pin_at_row(2, 0).unwrap();
    list.split(list.pin_at_row(1, 0).unwrap()).unwrap();
    let bottom = list.pin_at_row(2, bottom.x).unwrap();
    for (direction, start, limit, expected_rows, expected_cells) in [
        (
            Direction::RightDown,
            first,
            bottom,
            vec![(0, 0), (0, 1), (0, 2)],
            vec![(1, 0), (0, 1), (1, 1), (0, 2), (1, 2)],
        ),
        (
            Direction::LeftUp,
            bottom,
            first,
            vec![(0, 2), (0, 1), (0, 0)],
            vec![(0, 2), (1, 1), (0, 1), (1, 0), (0, 0)],
        ),
    ] {
        let mut rows = start.row_iterator(direction, Some(limit));
        let mut actual_rows = Vec::new();
        while let Some(pin) = rows.next(&list) {
            let point = list
                .point_from_pin(crate::point::Tag::Screen, pin)
                .unwrap()
                .coord();
            actual_rows.push((point.x, point.y));
        }
        assert_eq!(actual_rows, expected_rows);
        let mut cells = start.cell_iterator(direction, Some(limit));
        let mut actual_cells = Vec::new();
        while let Some(pin) = cells.next(&list) {
            let point = list
                .point_from_pin(crate::point::Tag::Screen, pin)
                .unwrap()
                .coord();
            actual_cells.push((point.x, point.y));
        }
        assert_eq!(actual_cells, expected_cells);
    }
}

#[test]
fn reverse_row_rotation_visits_only_the_affected_suffix() {
    let small = reverse_rotation_work(64, 64);
    let large = reverse_rotation_work(128, 64);
    let history = reverse_rotation_work(64, 128);
    assert_eq!(large.0 + 3, (small.0 + 3) * 2);
    assert_eq!(history.0, small.0);
    assert!(large.1 > small.1 * 3);
}

/// Count actual tail-to-cursor reads against a repeated-prefix negative control.
fn reverse_rotation_work(rows: u16, history: u16) -> (usize, usize) {
    let mut list = screen(5, rows + history);
    for y in (16..rows + history).step_by(16) {
        let pin = list.pin_at_row(usize::from(y), 0).unwrap();
        list.split(pin).unwrap();
    }
    let origin = list.pin_at_row(usize::from(history), 0).unwrap();
    let cursor = list.track_pin(origin).unwrap();
    crate::tests::WORK.with_borrow_mut(|work| work.queries.clear());
    let origin = *list.tracked_pin(cursor).unwrap();
    list.rotate_rows_down_from(origin).unwrap();
    let reverse = crate::tests::WORK
        .with_borrow(|work| work.queries.get(&list.first.arena).copied().unwrap_or(0));
    crate::tests::WORK.with_borrow_mut(|work| work.queries.clear());
    for target in 0..rows / 16 {
        let mut node = origin.node;
        for _ in 0..target {
            node = list.nodes.get(node).unwrap().next.unwrap();
        }
        assert!(list.page(node).is_some());
    }
    let repeated = crate::tests::WORK
        .with_borrow(|work| work.queries.get(&list.first.arena).copied().unwrap_or(0));
    eprintln!("ROTATION rows={rows} history={history} reverse={reverse} repeated={repeated}");
    (reverse, repeated)
}

#[test]
fn work_observations_keep_arena_owners_separate() {
    let mut list = screen(5, 3);
    let other = screen(5, 3);
    let origin = list.get_top_left(crate::point::Tag::Screen);
    let foreign = other.get_top_left(crate::point::Tag::Screen);
    let tracked = list.track_pin(origin).unwrap();
    crate::tests::WORK.with_borrow_mut(|work| work.queries.clear());
    assert!(list.page(origin.node).is_some());
    assert_eq!(*list.tracked_pin(tracked).unwrap(), origin);
    assert!(list.page(foreign.node).is_none());
    assert!(other.page(foreign.node).is_some());
    let queries = crate::tests::WORK.with_borrow(|work| work.queries.clone());
    let selected = queries.get(&origin.node.arena).copied().unwrap_or(0);
    assert_eq!(selected, 2);
    assert_eq!(queries.get(&tracked.0.arena), Some(&1));
    assert_eq!(queries.get(&foreign.node.arena), Some(&1));
    assert_eq!(queries.len(), 3);
    assert_ne!(queries.values().sum::<usize>(), selected);
    crate::tests::WORK.with_borrow_mut(|work| work.queries.clear());
    assert!(list.page(origin.node).is_some());
    crate::tests::WORK.with_borrow(|work| {
        assert_eq!(work.queries.get(&origin.node.arena), Some(&1));
        assert_eq!(work.queries.len(), 1);
    });
}
