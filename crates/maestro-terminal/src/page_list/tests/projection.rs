//! Initialization and absolute coordinates across retained page metadata.

use super::resize::screen;
use super::*;
use crate::point::Tag;

#[test]
fn history_page_list() {
    let mut list = screen(80, 24);
    assert_eq!(list.viewport, Viewport::Active);
    assert_eq!(list.total_pages(), 1);
    assert_eq!(list.total_rows(), 24);
    let origin = Pin {
        node: list.first,
        x: 0,
        y: 0,
        garbage: false,
    };
    assert_eq!(list.tracked_pin(list.viewport_pin), Some(&origin));
    assert_eq!(list.get_top_left(Tag::Active), origin);
    assert_eq!(
        list.scrollbar(),
        Scrollbar {
            total: 24,
            offset: 0,
            len: 24
        }
    );
}

#[test]
fn history_init_rows_across_two_pages() {
    let mut cols = 50;
    while PageList::initial_capacity(cols).rows >= 100 {
        cols += 50;
    }
    let mut list = screen(cols, 100);
    assert!(list.total_pages() >= 2);
    assert_eq!(list.viewport, Viewport::Active);
    assert_eq!(list.total_rows(), 100);
    assert_eq!(
        list.scrollbar(),
        Scrollbar {
            total: 100,
            offset: 0,
            len: 100
        }
    );
}

#[test]
fn history_init_more_than_max_cols() {
    let cols = STD_CAPACITY.max_cols().unwrap() + 1;
    let mut list = screen(cols, 80);
    assert_eq!(list.viewport, Viewport::Active);
    assert!(
        PageList::page_charge(list.page(list.first).unwrap().capacity) > PageList::standard_bytes()
    );
    assert_eq!(list.total_rows(), 80);
    assert_eq!(
        list.scrollbar(),
        Scrollbar {
            total: 80,
            offset: 0,
            len: 80
        }
    );
}

#[test]
fn history_point_from_pin_active_no_history() {
    let list = screen(80, 24);
    for (x, y) in [(0, 0), (4, 2)] {
        let pin = Pin {
            node: list.first,
            x,
            y,
            garbage: false,
        };
        assert_eq!(
            list.point_from_pin(Tag::Active, pin),
            Some(Point::Active(Coordinate { x, y: u32::from(y) }))
        );
    }
}

#[test]
fn history_point_from_pin_active_from_prior_page() {
    let mut list = screen(80, 24);
    let capacity = list.page(list.last).unwrap().capacity.rows;
    for _ in 0..usize::from(capacity) * 5 {
        list.grow();
    }
    let pin = Pin {
        node: list.last,
        x: 2,
        y: 0,
        garbage: false,
    };
    assert_eq!(
        list.point_from_pin(Tag::Active, pin),
        Some(Point::Active(Coordinate { x: 2, y: 0 }))
    );
    assert_eq!(
        list.point_from_pin(Tag::Active, list.get_top_left(Tag::Screen)),
        None
    );
}

#[test]
fn history_point_from_pin_traverse_pages() {
    let mut list = screen(80, 24);
    let capacity = list.page(list.last).unwrap().capacity.rows;
    for _ in 0..usize::from(capacity) * 2 {
        list.grow();
    }
    let pin = Pin {
        node: list.nodes.get(list.last).unwrap().prev.unwrap(),
        x: 2,
        y: 5,
        garbage: false,
    };
    let y = u32::try_from(usize::from(capacity) * (list.total_pages() - 2) + 5).unwrap();
    assert_eq!(
        list.point_from_pin(Tag::Screen, pin),
        Some(Point::Screen(Coordinate { x: 2, y }))
    );
    assert_eq!(
        list.point_from_pin(Tag::Active, list.get_top_left(Tag::Screen)),
        None
    );
}

/// Construct linked metadata without allocating the represented billions of cells.
fn metadata_pages(count: usize, rows: u16) -> PageList {
    let mut list = screen(1, 1);
    list.page_mut(list.first).unwrap().size.rows = rows;
    for _ in 1..count {
        let mut page = Page::init(Capacity {
            cols: 1,
            rows: 1,
            styles: 0,
            hyperlink_bytes: 0,
            grapheme_bytes: 0,
            string_bytes: 0,
        });
        page.size.rows = rows;
        let next = list.nodes.insert(Node {
            page,
            prev: Some(list.last),
            next: None,
        });
        list.nodes.get_mut(list.last).unwrap().next = Some(next);
        list.last = next;
    }
    list.total_rows = count * usize::from(rows);
    list
}

#[test]
fn history_point_from_pin_rejects_overflowing_screen_coordinate() {
    let list = metadata_pages(65_539, u16::MAX);
    assert!(list.total_rows() > usize::try_from(u32::MAX).unwrap());
    let pin = Pin {
        node: list.last,
        x: 0,
        y: 0,
        garbage: false,
    };
    assert!(list.pin_is_valid(pin));
    assert_eq!(list.point_from_pin(Tag::Screen, pin), None);
    assert_eq!(
        list.point_from_pin(Tag::Screen, list.get_top_left(Tag::Screen)),
        Some(Point::Screen(Coordinate::default()))
    );
}

#[test]
fn history_cell_screen_point_supports_long_scrollback() {
    let list = metadata_pages(307, STD_CAPACITY.rows);
    let y = 306 * u32::from(STD_CAPACITY.rows);
    assert!(y > u32::from(u16::MAX));
    let point = Point::Screen(Coordinate { x: 0, y });
    let cell = list.get_cell(point).unwrap();
    assert_eq!(cell.node, list.last);
    assert_eq!(
        list.point_from_pin(
            Tag::Screen,
            Pin {
                node: cell.node,
                x: cell.col_idx,
                y: cell.row_idx,
                garbage: false
            }
        ),
        Some(point)
    );
}

#[test]
fn history_active_after_grow() {
    let mut list = screen(80, 24);
    assert_eq!(list.total_rows(), usize::from(list.rows()));
    for _ in 0..10 {
        list.grow();
    }
    assert_eq!(list.total_rows(), 34);
    for (tag, y) in [(Tag::Viewport, 10), (Tag::Screen, 0), (Tag::Active, 10)] {
        let cell = list
            .get_cell(match tag {
                Tag::Viewport => Point::Viewport(Coordinate::default()),
                Tag::Screen => Point::Screen(Coordinate::default()),
                Tag::Active => Point::Active(Coordinate::default()),
                Tag::History => unreachable!(),
            })
            .unwrap();
        let pin = Pin {
            node: cell.node,
            x: cell.col_idx,
            y: cell.row_idx,
            garbage: false,
        };
        assert_eq!(
            list.point_from_pin(Tag::Screen, pin),
            Some(Point::Screen(Coordinate { x: 0, y }))
        );
    }
    assert_eq!(
        list.scrollbar(),
        Scrollbar {
            total: 34,
            offset: 10,
            len: 24
        }
    );
}
