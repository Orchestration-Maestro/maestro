//! Movement across unequal stored widths and arithmetic boundaries.

use super::resize::{nz, screen};
use super::*;

/// Three single-row pages whose widths deliberately differ from the desired width.
fn mixed_width() -> (PageList, [Pin; 3]) {
    let mut list = screen(2, 1);
    let first = list.first;
    let mut previous = first;
    let mut keys = vec![first];
    for cols in [4, 3] {
        let mut page = Page::init(PageList::initial_capacity(cols));
        page.size.rows = 1;
        let next = list.nodes.insert(Node {
            page,
            prev: Some(previous),
            next: None,
        });
        list.nodes.get_mut(previous).unwrap().next = Some(next);
        list.page_size += PageList::page_charge(list.page(next).unwrap().capacity);
        list.total_rows += 1;
        previous = next;
        keys.push(next);
    }
    list.last = previous;
    list.cols = nz(4);
    let pins = std::array::from_fn(|i| Pin {
        node: keys[i],
        x: 0,
        y: 0,
        garbage: false,
    });
    (list, pins)
}

#[test]
fn history_pin_row_movement_clamps_across_mixed_width_pages() {
    let (list, [first, second, third]) = mixed_width();
    let start = Pin { x: 3, ..second };
    let top = Pin { x: 1, ..first };
    let bottom = Pin { x: 2, ..third };
    assert_eq!(start.down(&list, 1), Some(bottom));
    assert_eq!(start.up(&list, 1), Some(top));
    assert_eq!(
        start.down_overflow(&list, 10),
        Some(Offset::Overflow {
            end: bottom,
            remaining: 9
        })
    );
    assert_eq!(
        start.up_overflow(&list, 10),
        Some(Offset::Overflow {
            end: top,
            remaining: 9
        })
    );
}

#[test]
fn history_pin_wrapping_crosses_mixed_width_pages() {
    let (list, [first, second, third]) = mixed_width();
    assert_eq!(
        (Pin { x: 1, ..first }).right_wrap(&list, 7),
        Some(Pin { x: 2, ..third })
    );
    assert_eq!((Pin { x: 2, ..third }).left_wrap(&list, 6), Some(second));
    assert_eq!(first.left_wrap(&list, 1), None);
    assert_eq!((Pin { x: 2, ..third }).right_wrap(&list, 1), None);
}

#[test]
fn history_pin_rejects_columns_beyond_mixed_width_page_bounds() {
    let (list, _) = mixed_width();
    for (x, y, valid) in [(1, 0, true), (2, 0, false), (3, 1, true), (3, 2, false)] {
        assert_eq!(
            list.pin(Point::Screen(Coordinate { x, y })).is_some(),
            valid
        );
    }
}

#[test]
fn history_pin_right_wrap_exact_row_multiple() {
    let list = screen(10, 3);
    let start = list.pin(Point::Active(Coordinate { x: 5, y: 0 })).unwrap();
    let wrapped = start.right_wrap(&list, 14).unwrap();
    assert!(wrapped.row_and_cell(&list).is_some());
    assert_eq!(
        list.point_from_pin(crate::point::Tag::Active, wrapped),
        Some(Point::Active(Coordinate { x: 9, y: 1 }))
    );
}

#[test]
fn history_pin_left_wrap_exact_row_multiple() {
    let list = screen(10, 3);
    let start = list.pin(Point::Active(Coordinate { x: 5, y: 2 })).unwrap();
    let wrapped = start.left_wrap(&list, 15).unwrap();
    assert!(wrapped.row_and_cell(&list).is_some());
    assert_eq!(
        list.point_from_pin(crate::point::Tag::Active, wrapped),
        Some(Point::Active(Coordinate { x: 0, y: 1 }))
    );
}

#[test]
fn history_pin_right_wrap_maximum_distance() {
    let list = screen(1, 3);
    assert_eq!(
        list.get_top_left(crate::point::Tag::Active)
            .right_wrap(&list, usize::MAX),
        None
    );
}

#[test]
fn history_pin_left_wrap_maximum_distance() {
    let list = screen(1, 3);
    assert_eq!(
        list.get_bottom_right(crate::point::Tag::Active)
            .unwrap()
            .left_wrap(&list, usize::MAX),
        None
    );
}
