//! History coordinates and retained cell positions.

use super::support::LARGE_PAGES;
use maestro_terminal::{Coordinate, Point};

/// Recorded bytes of one standard page for 4 KiB or 16 KiB host pages.
const STANDARD_PAGE: u64 = if LARGE_PAGES { 409_600 } else { 401_408 };

#[test]
fn point_regions_and_coordinate_boundaries() {
    for x in [0, 1, u16::MAX] {
        for y in [0, 1, u32::MAX] {
            let coordinate = Coordinate { x, y };
            for point in [
                Point::Active(coordinate),
                Point::Viewport(coordinate),
                Point::Screen(coordinate),
                Point::History(coordinate),
            ] {
                assert_eq!(point.coord(), coordinate);
            }
            assert!(coordinate.eql(coordinate));
            assert!(!coordinate.eql(Coordinate { x: x ^ 1, y }));
            assert!(!coordinate.eql(Coordinate { x, y: y ^ 1 }));
        }
    }
    let list = history_list(5, 3);
    assert!(
        list.get_bottom_right(maestro_terminal::point::Tag::History)
            .is_none()
    );
    let bottom = list
        .get_bottom_right(maestro_terminal::point::Tag::Active)
        .unwrap();
    assert_eq!(
        list.point_from_pin(maestro_terminal::point::Tag::Active, bottom),
        Some(Point::Active(Coordinate { x: 4, y: 2 }))
    );
    assert_eq!(bottom.row_and_cell(&list).unwrap().1.codepoint(), '\0');
}

#[test]
fn initial_capacity_and_minimum_budget() {
    use maestro_terminal::PageList;
    use std::num::NonZeroU16;
    // Rows of one standard page for 4 KiB and for 16 KiB host pages.
    for (cols, small, large) in [
        (1, 23_454, 23_966),
        (5, 7818, 7988),
        (80, 579, 591),
        (215, 217, 221),
        (430, 108, 111),
        (65_535, 215, 215),
    ] {
        let capacity_rows = if LARGE_PAGES { large } else { small };
        for rows in [1, 24, 216] {
            if cols == u16::MAX && rows != 1 {
                continue;
            }
            assert_initial_capacity(cols, rows, capacity_rows);
        }
    }
    // The effective limit never falls below two standard pages.
    for (limit, small, large) in [
        (None, u64::MAX, u64::MAX),
        (Some(0), 802_816, 819_200),
        (Some(1), 802_816, 819_200),
        (Some(802_815), 802_816, 819_200),
        (Some(802_816), 802_816, 819_200),
        (Some(1_204_224), 1_204_224, 1_204_224),
    ] {
        let list = PageList::init(
            NonZeroU16::new(80).unwrap(),
            NonZeroU16::new(24).unwrap(),
            limit,
        );
        assert_eq!(list.max_size(), if LARGE_PAGES { large } else { small });
        assert_eq!(list.charged_bytes(), STANDARD_PAGE);
    }
}

#[test]
fn pin_handles_reject_stale_foreign_and_released_keys() {
    use maestro_terminal::PageList;
    use std::num::NonZeroU16;
    let dimension = NonZeroU16::new(2).unwrap();
    let mut list = PageList::init(dimension, dimension, None);
    let other = PageList::init(dimension, dimension, None);
    let pin = list.pin(Point::Screen(Coordinate::default())).unwrap();
    let foreign = other.pin(Point::Screen(Coordinate::default())).unwrap();
    assert!(!list.node_is_valid(foreign.node));
    assert!(!list.pin_is_valid(foreign));
    let retained = list.track_pin(pin).unwrap();
    list.untrack_pin(retained);
    let replacement = list.track_pin(pin).unwrap();
    assert_ne!(retained, replacement);
    assert!(list.tracked_pin(retained).is_none());
    list.untrack_pin(retained);
    assert_eq!(list.tracked_pin(replacement), Some(&pin));
    list.reset();
    assert!(!list.node_is_valid(pin.node));
    assert!(pin.row_and_cell(&list).is_none());
    let moved = *list.tracked_pin(replacement).unwrap();
    assert!(moved.garbage);
    assert_eq!(
        moved,
        list.pin(Point::Active(Coordinate::default())).unwrap()
    );
    assert!(list.pin_is_valid(moved));
    list.untrack_pin(replacement);
    assert_eq!(list.count_tracked_pins(), 1);
    check_page_key_replacement(&mut list);
}

/// Replaced and removed page slots never revive an old key.
fn check_page_key_replacement(list: &mut maestro_terminal::PageList) {
    use maestro_terminal::point::Tag;
    let pin = list.get_top_left(Tag::Screen);
    let replacement = list.increase_capacity(pin.node, None).unwrap();
    assert_ne!(pin.node, replacement);
    assert!(!list.node_is_valid(pin.node));
    assert!(pin.row_and_cell(list).is_none());
    let split = list.pin(Point::Screen(Coordinate { x: 0, y: 1 })).unwrap();
    list.split(split).unwrap();
    let removed = list.get_top_left(Tag::Screen).node;
    list.grow();
    list.erase_history(None);
    assert!(!list.node_is_valid(removed));
    let live = list.get_top_left(Tag::Screen);
    list.increase_capacity(live.node, None).unwrap();
    assert!(!list.node_is_valid(removed));
    assert!(!list.node_is_valid(pin.node));
}

#[test]
fn pin_membership_includes_both_endpoints() {
    for pages in 1..=3 {
        let list = membership_list(pages);
        for (top, bottom) in [
            (Coordinate { x: 1, y: 0 }, Coordinate { x: 3, y: 2 }),
            (Coordinate { x: 1, y: 1 }, Coordinate { x: 3, y: 1 }),
            (Coordinate { x: 2, y: 1 }, Coordinate { x: 2, y: 1 }),
        ] {
            let first = list.pin(Point::Screen(top)).unwrap();
            let endpoint = list.pin(Point::Screen(bottom)).unwrap();
            for (x, y) in (0..3).flat_map(|y| (0..5).map(move |x| (x, y))) {
                let pin = list.pin(Point::Screen(Coordinate { x, y })).unwrap();
                assert_eq!(
                    pin.is_between(&list, first, endpoint),
                    (y, x) >= (top.y, top.x) && (y, x) <= (bottom.y, bottom.x)
                );
                assert_eq!(
                    pin,
                    maestro_terminal::Pin {
                        garbage: false,
                        ..pin
                    }
                );
                assert_eq!(
                    pin,
                    maestro_terminal::Pin {
                        garbage: true,
                        ..pin
                    }
                );
            }
        }
    }
}

#[test]
fn pin_membership_rejects_outside_neighbours() {
    for pages in 1..=3 {
        let list = membership_list(pages);
        for (top, bottom) in [
            (Coordinate { x: 1, y: 0 }, Coordinate { x: 3, y: 2 }),
            (Coordinate { x: 1, y: 1 }, Coordinate { x: 3, y: 1 }),
            (Coordinate { x: 2, y: 1 }, Coordinate { x: 2, y: 1 }),
        ] {
            let first = list.pin(Point::Screen(top)).unwrap();
            let endpoint = list.pin(Point::Screen(bottom)).unwrap();
            for (x, y) in (0..3).flat_map(|y| (0..5).map(move |x| (x, y))) {
                let pin = list.pin(Point::Screen(Coordinate { x, y })).unwrap();
                assert_eq!(pin.before(&list, first), (y, x) < (top.y, top.x));
                assert_eq!(endpoint.before(&list, pin), (y, x) > (bottom.y, bottom.x));
                assert_eq!(
                    !pin.is_between(&list, first, endpoint),
                    (y, x) < (top.y, top.x) || (y, x) > (bottom.y, bottom.x)
                );
            }
            if first != endpoint {
                assert!(!first.is_between(&list, endpoint, first));
            }
        }
    }
}

#[test]
fn pin_movement_and_clamping_boundaries() {
    use maestro_terminal::{PageList, page_list::Offset};
    use std::num::NonZeroU16;
    let mut list = PageList::init(
        NonZeroU16::new(5).unwrap(),
        NonZeroU16::new(3).unwrap(),
        None,
    );
    let split = list.pin(Point::Screen(Coordinate { x: 0, y: 1 })).unwrap();
    list.split(split).unwrap();
    let start = list.pin(Point::Screen(Coordinate { x: 1, y: 0 })).unwrap();
    let end = list.pin(Point::Screen(Coordinate { x: 4, y: 2 })).unwrap();
    assert_eq!(start.left(1).x, 0);
    assert_eq!(start.right(&list, 3).unwrap().x, 4);
    assert!(start.right(&list, 4).is_none());
    assert_eq!(start.left_clamp(u16::MAX).x, 0);
    assert_eq!(start.right_clamp(&list, u16::MAX).unwrap().x, 4);
    assert!(start.up(&list, 1).is_none());
    assert!(end.down(&list, 1).is_none());
    assert_wrapping_boundaries(&list, start, end);
    movement_corpus(&list);
    assert_eq!(start.down(&list, 2).unwrap().y, 1);
    assert_eq!(end.up(&list, 2).unwrap().y, 0);
    assert_eq!(
        start.up_overflow(&list, 7),
        Some(Offset::Overflow {
            end: start,
            remaining: 7
        })
    );
    assert_eq!(
        end.down_overflow(&list, 7),
        Some(Offset::Overflow { end, remaining: 7 })
    );
}

/// Check wrap distances independently of vertical and in-row movement.
fn assert_wrapping_boundaries(
    list: &maestro_terminal::PageList,
    start: maestro_terminal::Pin,
    end: maestro_terminal::Pin,
) {
    assert_eq!(start.right_wrap(list, 13), Some(end));
    assert_eq!(end.left_wrap(list, 13), Some(start));
    assert!(start.right_wrap(list, usize::MAX).is_none());
    assert!(end.left_wrap(list, usize::MAX).is_none());
}

#[test]
fn mutable_iteration_reuses_the_owned_page() {
    use maestro_terminal::{PageList, page_list::Direction};
    use std::num::NonZeroU16;
    let two = NonZeroU16::new(2).unwrap();
    let mut list = PageList::init(two, two, None);
    let split = list.pin(Point::Screen(Coordinate { x: 0, y: 1 })).unwrap();
    list.split(split).unwrap();
    for y in 0..2 {
        let pin = list.pin(Point::Screen(Coordinate { x: 0, y })).unwrap();
        list.page_mut(pin.node).unwrap().get_cells_mut(pin.y)[0].set_content(
            maestro_terminal::page::cell::Content::Codepoint(if y == 0 { 'A' } else { 'B' }),
        );
    }
    let mut iterator = list.page_iterator(
        Direction::RightDown,
        Point::Screen(Coordinate::default()),
        None,
    );
    let mut visited = Vec::new();
    while let Some(chunk) = iterator.next(&list) {
        let page = list.page_mut(chunk.node).unwrap();
        visited.push((chunk.node, page.get_cells(chunk.start)[0].codepoint()));
        for y in chunk.start..chunk.end {
            page.clear_cells(y, 0..page.size.cols);
        }
    }
    assert_eq!(visited.len(), 2);
    assert_ne!(visited[0].0, visited[1].0);
    assert_eq!([visited[0].1, visited[1].1], ['A', 'B']);
    assert!(iterator.next(&list).is_none());
    for y in 0..2 {
        assert_eq!(
            list.get_cell(Point::Screen(Coordinate { x: 0, y }))
                .unwrap()
                .cell
                .codepoint(),
            '\0'
        );
    }
}

#[test]
fn pin_dirty_marks_keep_page_and_row_flags_independent() {
    use maestro_terminal::PageList;
    use std::num::NonZeroU16;
    let two = NonZeroU16::new(2).unwrap();
    let mut list = PageList::init(two, two, None);
    let pin = list.pin(Point::Screen(Coordinate::default())).unwrap();
    for page_dirty in [false, true] {
        for row_dirty in [false, true] {
            let page = list.page_mut(pin.node).unwrap();
            page.dirty = page_dirty;
            page.get_row_mut(0).set_dirty(row_dirty);
            assert_eq!(pin.is_dirty(&list), Some(page_dirty || row_dirty));
        }
    }
    let page = list.page_mut(pin.node).unwrap();
    page.dirty = false;
    page.get_row_mut(0).set_dirty(false);
    pin.mark_dirty(&mut list);
    let page = list.page(pin.node).unwrap();
    assert!(!page.dirty);
    assert!(page.get_row(0).dirty());
    assert!(!page.get_row(1).dirty());
    list.reset();
    assert_eq!(pin.is_dirty(&list), None);
    let clean = list.pin(Point::Screen(Coordinate::default())).unwrap();
    assert_eq!(clean.is_dirty(&list), Some(false));
    let page = list.page_mut(clean.node).unwrap();
    page.get_cells_mut(0)[0].set_content(maestro_terminal::page::cell::Content::Codepoint('e'));
    page.append_grapheme(
        maestro_terminal::page::CellPosition { x: 0, y: 0 },
        '\u{301}',
    )
    .unwrap();
    assert_eq!(clean.is_dirty(&list), Some(false));
}

/// Construct a public list with small active dimensions.
fn history_list(cols: u16, rows: u16) -> maestro_terminal::PageList {
    maestro_terminal::PageList::init(
        std::num::NonZeroU16::new(cols).unwrap(),
        std::num::NonZeroU16::new(rows).unwrap(),
        None,
    )
}

/// Divide a three-row grid into one, two or three ordered pages.
fn membership_list(pages: u32) -> maestro_terminal::PageList {
    let mut list = history_list(5, 3);
    for y in 1..pages {
        let pin = list.pin(Point::Screen(Coordinate { x: 0, y })).unwrap();
        list.split(pin).unwrap();
    }
    assert_eq!(list.total_pages(), usize::try_from(pages).unwrap());
    list
}

#[test]
fn tracked_pin_edits_are_visible_after_list_changes() {
    use maestro_terminal::page_list::IncreaseCapacity;
    use maestro_terminal::point::Tag;
    let mut list = history_list(5, 3);
    let other = history_list(5, 3);
    let origin = list.get_top_left(Tag::Screen);
    let released = list.track_pin(origin).unwrap();
    let retained = list.track_pin(origin).unwrap();
    list.untrack_pin(released);
    let edited = list.pin(Point::Screen(Coordinate { x: 2, y: 1 })).unwrap();
    *list.tracked_pin_mut(retained).unwrap() = edited;
    edited
        .row_and_cell_mut(&mut list)
        .unwrap()
        .1
        .set_content(maestro_terminal::page::cell::Content::Codepoint('R'));
    assert_retained(&list, retained, Coordinate { x: 2, y: 1 }, 'R', false);
    list.grow();
    assert_retained(&list, retained, Coordinate { x: 2, y: 1 }, 'R', false);
    let old = list.tracked_pin(retained).unwrap().node;
    let new = list
        .increase_capacity(old, Some(IncreaseCapacity::Styles))
        .unwrap();
    assert_ne!(old, new);
    assert!(list.page(old).is_none());
    assert_retained(&list, retained, Coordinate { x: 2, y: 1 }, 'R', false);
    list.split(*list.tracked_pin(retained).unwrap()).unwrap();
    assert_ne!(list.tracked_pin(retained).unwrap().node, new);
    assert_retained(&list, retained, Coordinate { x: 2, y: 1 }, 'R', false);
    list.erase_history(None);
    assert_retained(&list, retained, Coordinate { x: 2, y: 0 }, 'R', false);
    list.erase_row(Point::Screen(Coordinate::default()))
        .unwrap();
    assert_retained(&list, retained, Coordinate { x: 2, y: 0 }, '\0', false);
    assert!(list.tracked_pin(released).is_none());
    assert!(other.tracked_pin(retained).is_none());
    assert_eq!(
        other
            .get_cell(Point::Screen(Coordinate::default()))
            .unwrap()
            .cell
            .codepoint(),
        '\0'
    );
    list.reset();
    assert_retained(&list, retained, Coordinate::default(), '\0', true);
    assert!(list.tracked_pin(released).is_none());
    assert_eq!(list.count_tracked_pins(), 2);
    list.untrack_pin(retained);
    assert!(list.tracked_pin(retained).is_none());
    assert_eq!(list.count_tracked_pins(), 1);
}

/// Read the same retained slot after a list edit, including nonlocation state.
fn assert_retained(
    list: &maestro_terminal::PageList,
    handle: maestro_terminal::page_list::TrackedPin,
    coordinate: Coordinate,
    codepoint: char,
    garbage: bool,
) {
    let pin = *list.tracked_pin(handle).unwrap();
    assert_eq!(
        list.point_from_pin(maestro_terminal::point::Tag::Screen, pin),
        Some(Point::Screen(coordinate))
    );
    assert_eq!(pin.garbage, garbage);
    assert_eq!(pin.row_and_cell(list).unwrap().1.codepoint(), codepoint);
}

#[test]
fn scrollbar_offsets_follow_every_edit() {
    use maestro_terminal::page_list::{Resize, Scroll};
    for view in [Scroll::Top, Scroll::Active, Scroll::Row(4)] {
        let mut list = history_list(5, 3);
        for _ in 0..10 {
            list.grow();
        }
        list.scroll(view);
        let offset = match view {
            Scroll::Top => 0,
            Scroll::Active => 10,
            _ => 4,
        };
        assert_bar(&mut list, (13, offset, 3));
        list.scroll(Scroll::Row(2));
        assert_bar(&mut list, (13, 2, 3));
        list.scroll(Scroll::Row(7));
        assert_bar(&mut list, (13, 7, 3));
        list.scroll(Scroll::Row(3));
        assert_bar(&mut list, (13, 3, 3));
        list.scroll(Scroll::DeltaRow(3));
        assert_bar(&mut list, (13, 6, 3));
        list.scroll(Scroll::DeltaRow(-2));
        assert_bar(&mut list, (13, 4, 3));
        scrollbar_list_edits(&mut list);
        for y in 0..u32::try_from(list.total_rows()).unwrap() {
            let pin = list.pin(Point::Screen(Coordinate { x: 0, y })).unwrap();
            pin.row_and_cell_mut(&mut list)
                .unwrap()
                .1
                .set_content(maestro_terminal::page::cell::Content::Codepoint('X'));
        }
        list.resize(Resize {
            cols: std::num::NonZeroU16::new(6),
            reflow: false,
            ..Resize::default()
        })
        .unwrap();
        assert_bar(&mut list, (10, 2, 3));
        list.resize(Resize {
            cols: std::num::NonZeroU16::new(5),
            ..Resize::default()
        })
        .unwrap();
        assert_bar(&mut list, (10, 2, 3));
        list.scroll(Scroll::Active);
        assert_bar(&mut list, (10, 7, 3));
        list.scroll(Scroll::Top);
        assert_bar(&mut list, (10, 0, 3));
        for y in [2, 5] {
            let split = list.pin(Point::Screen(Coordinate { x: 0, y })).unwrap();
            list.split(split).unwrap();
        }
        list.erase_history(None);
        assert_bar(&mut list, (3, 0, 3));
        list.erase_active(2);
        assert_bar(&mut list, (3, 0, 3));
    }
    scrollbar_pruning();
    scrollbar_following_edits(Scroll::Top);
    scrollbar_following_edits(Scroll::Active);
}

/// Exercise cached positions through replacement, split and each erasure owner.
fn scrollbar_list_edits(list: &mut maestro_terminal::PageList) {
    use maestro_terminal::page_list::Scroll;
    let pin = list.pin(Point::Screen(Coordinate { x: 0, y: 3 })).unwrap();
    list.increase_capacity(pin.node, None).unwrap();
    assert_bar(list, (13, 4, 3));
    let pin = list.pin(Point::Screen(Coordinate { x: 0, y: 3 })).unwrap();
    list.split(pin).unwrap();
    assert_bar(list, (13, 4, 3));
    list.erase_row(Point::Screen(Coordinate { x: 0, y: 2 }))
        .unwrap();
    assert_bar(list, (13, 3, 3));
    list.erase_row_bounded(Point::Screen(Coordinate { x: 0, y: 2 }), 3)
        .unwrap();
    assert_bar(list, (13, 2, 3));
    list.erase_history(Some(Point::History(Coordinate { x: 0, y: 0 })));
    assert_bar(list, (12, 1, 3));
    list.scroll(Scroll::Row(3));
    list.erase_history(Some(Point::History(Coordinate { x: 0, y: 0 })));
    assert_bar(list, (11, 2, 3));
    list.erase_active(0);
    assert_bar(list, (11, 2, 3));
    list.erase_history(Some(Point::History(Coordinate { x: 0, y: 0 })));
    assert_bar(list, (10, 1, 3));
    list.scroll(Scroll::Row(2));
    assert_bar(list, (10, 2, 3));
}

/// Check pruning observations for all three viewport modes at a real page boundary.
fn scrollbar_pruning() {
    use maestro_terminal::page_list::Scroll;
    use maestro_terminal::point::Tag;
    for view in [Scroll::Top, Scroll::Active, Scroll::Row(4)] {
        let mut list = maestro_terminal::PageList::init(
            std::num::NonZeroU16::new(80).unwrap(),
            std::num::NonZeroU16::new(24).unwrap(),
            Some(802_816),
        );
        let first = list.get_top_left(Tag::Screen).node;
        let rows = list.page(first).unwrap().capacity.rows;
        for _ in 24..usize::from(rows) * 2 {
            list.grow();
        }
        assert_eq!(list.total_pages(), 2);
        list.scroll(view);
        let offset = match view {
            Scroll::Top => 0,
            Scroll::Active => usize::from(rows) * 2 - 24,
            _ => 4,
        };
        assert_bar(&mut list, (usize::from(rows) * 2, offset, 24));
        list.grow();
        assert!(!list.node_is_valid(first));
        let offset = if view == Scroll::Active {
            usize::from(rows) + 1 - 24
        } else {
            0
        };
        assert_bar(&mut list, (usize::from(rows) + 1, offset, 24));
    }
}

/// Read twice so a stale warmed cache cannot hide behind one uncached result.
fn assert_bar(list: &mut maestro_terminal::PageList, expected: (usize, usize, usize)) {
    for _ in 0..2 {
        let bar = list.scrollbar();
        assert_eq!((bar.total, bar.offset, bar.len), expected);
    }
}

/// Compare every capacity field and logical charge in one initial fixture.
fn assert_initial_capacity(cols: u16, rows: u16, capacity_rows: u16) {
    use maestro_terminal::{
        Page, PageList,
        page::layout::{Capacity, STD_CAPACITY},
    };
    use std::num::NonZeroU16;
    let list = PageList::init(
        NonZeroU16::new(cols).unwrap(),
        NonZeroU16::new(rows).unwrap(),
        Some(0),
    );
    let origin = list.get_top_left(maestro_terminal::point::Tag::Screen);
    let capacity = Capacity {
        cols,
        rows: capacity_rows,
        ..STD_CAPACITY
    };
    assert_eq!(list.page(origin.node).unwrap().capacity, capacity);
    let pages = usize::from(rows).div_ceil(usize::from(capacity_rows));
    assert_eq!(
        (
            list.cols(),
            list.rows(),
            list.total_rows(),
            list.total_pages()
        ),
        (cols, rows, usize::from(rows), pages)
    );
    assert_eq!(
        list.charged_bytes(),
        Page::layout(capacity).total_size * u64::try_from(pages).unwrap()
    );
    assert_eq!(
        list.max_size(),
        STANDARD_PAGE * u64::try_from(pages + 1).unwrap()
    );
    assert_eq!(list.count_tracked_pins(), 1);
    assert_eq!(origin.row_and_cell(&list).unwrap().1.codepoint(), '\0');
    assert!(
        list.pin(Point::Screen(Coordinate { x: cols, y: 0 }))
            .is_none()
    );
    assert!(
        list.pin(Point::Screen(Coordinate {
            x: 0,
            y: u32::from(rows)
        }))
        .is_none()
    );
}

/// Resolve one row-major coordinate only when the arithmetic stays in the grid.
fn linear_pin(
    list: &maestro_terminal::PageList,
    offset: Option<usize>,
) -> Option<maestro_terminal::Pin> {
    let offset = offset.filter(|&value| value < 15)?;
    list.pin(Point::Screen(Coordinate {
        x: u16::try_from(offset % 5).unwrap(),
        y: u32::try_from(offset / 5).unwrap(),
    }))
}

/// Check the complete movement operand corpus against independent grid arithmetic.
fn movement_corpus(list: &maestro_terminal::PageList) {
    for x in [0, 2, 4] {
        for distance in [0, 1, 4, 5, 10, usize::MAX] {
            movement_case(list, x, distance);
        }
    }
}

/// Compare horizontal, vertical and overflow movement at one interior row.
fn movement_case(list: &maestro_terminal::PageList, x: u16, distance: usize) {
    use maestro_terminal::page_list::Offset;
    let pin = list.pin(Point::Screen(Coordinate { x, y: 1 })).unwrap();
    let offset = 5 + usize::from(x);
    assert_eq!(
        pin.left_wrap(list, distance),
        linear_pin(list, offset.checked_sub(distance))
    );
    assert_eq!(
        pin.right_wrap(list, distance),
        linear_pin(list, offset.checked_add(distance))
    );
    let clamp = u16::try_from(distance).unwrap_or(u16::MAX);
    assert_eq!(pin.left_clamp(clamp).x, x.saturating_sub(clamp));
    assert_eq!(
        pin.right_clamp(list, clamp).unwrap().x,
        x.saturating_add(clamp).min(4)
    );
    if distance <= usize::from(x) {
        assert_eq!(
            pin.left(distance),
            linear_pin(list, offset.checked_sub(distance)).unwrap()
        );
    }
    assert_eq!(
        pin.right(list, distance),
        offset
            .checked_add(distance)
            .filter(|&value| value < 10)
            .and_then(|value| linear_pin(list, Some(value)))
    );
    let above = 1_usize
        .checked_sub(distance)
        .and_then(|y| linear_pin(list, Some(y * 5 + usize::from(x))));
    let below = 1_usize
        .checked_add(distance)
        .filter(|&y| y < 3)
        .and_then(|y| linear_pin(list, Some(y * 5 + usize::from(x))));
    assert_eq!(pin.up(list, distance), above);
    assert_eq!(pin.down(list, distance), below);
    let expected_up = above.map_or_else(
        || Offset::Overflow {
            end: linear_pin(list, Some(usize::from(x))).unwrap(),
            remaining: distance - 1,
        },
        Offset::Offset,
    );
    let expected_down = below.map_or_else(
        || Offset::Overflow {
            end: linear_pin(list, Some(10 + usize::from(x))).unwrap(),
            remaining: distance - 1,
        },
        Offset::Offset,
    );
    assert_eq!(pin.up_overflow(list, distance), Some(expected_up));
    assert_eq!(pin.down_overflow(list, distance), Some(expected_down));
}

/// Keep top and active following modes through the same mutation owners as pins.
fn scrollbar_following_edits(view: maestro_terminal::page_list::Scroll) {
    use maestro_terminal::page_list::Resize;
    let mut list = history_list(5, 3);
    for _ in 0..10 {
        list.grow();
    }
    list.scroll(view);
    let pin = list.pin(Point::Screen(Coordinate { x: 0, y: 3 })).unwrap();
    list.split(pin).unwrap();
    assert_following_bar(&mut list, view, 13);
    let pin = list.pin(Point::Screen(Coordinate::default())).unwrap();
    list.increase_capacity(pin.node, None).unwrap();
    assert_following_bar(&mut list, view, 13);
    list.erase_row(Point::Screen(Coordinate { x: 0, y: 2 }))
        .unwrap();
    assert_following_bar(&mut list, view, 13);
    list.erase_row_bounded(Point::Screen(Coordinate { x: 0, y: 2 }), 3)
        .unwrap();
    assert_following_bar(&mut list, view, 13);
    list.erase_history(Some(Point::History(Coordinate::default())));
    assert_following_bar(&mut list, view, 12);
    list.erase_active(0);
    assert_following_bar(&mut list, view, 12);
    for y in 0..12 {
        let pin = list.pin(Point::Screen(Coordinate { x: 0, y })).unwrap();
        pin.row_and_cell_mut(&mut list)
            .unwrap()
            .1
            .set_content(maestro_terminal::page::cell::Content::Codepoint('X'));
    }
    for reflow in [false, true] {
        list.resize(Resize {
            cols: std::num::NonZeroU16::new(if reflow { 5 } else { 6 }),
            reflow,
            ..Resize::default()
        })
        .unwrap();
        assert_following_bar(&mut list, view, 12);
    }
    list.erase_history(None);
    assert_following_bar(&mut list, view, 3);
    list.erase_active(2);
    assert_following_bar(&mut list, view, 3);
}

/// Check a viewport mode that follows either end rather than a retained cell.
fn assert_following_bar(
    list: &mut maestro_terminal::PageList,
    view: maestro_terminal::page_list::Scroll,
    total: usize,
) {
    let offset = if view == maestro_terminal::page_list::Scroll::Active {
        total - 3
    } else {
        0
    };
    assert_bar(list, (total, offset, 3));
}
