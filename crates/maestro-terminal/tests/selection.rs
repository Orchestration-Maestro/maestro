//! Inclusive range geometry through retained screen pins.

use maestro_terminal::{
    Coordinate, Pin, Point, Screen,
    selection::{Order, Selection},
};

fn screen() -> Screen {
    Screen::init(maestro_terminal::screen::Options {
        cols: std::num::NonZeroU16::new(15).unwrap(),
        rows: std::num::NonZeroU16::new(15).unwrap(),
        max_scrollback: 0,
    })
}

fn pin(screen: &Screen, x: u16, y: u32) -> Pin {
    screen
        .pages
        .pin(Point::Screen(Coordinate { x, y }))
        .unwrap()
}

fn selection(screen: &Screen, start: (u16, u32), end: (u16, u32), rectangle: bool) -> Selection {
    Selection::init(
        pin(screen, start.0, start.1),
        pin(screen, end.0, end.1),
        rectangle,
    )
}

#[test]
fn selection_order_standard() {
    let screen = screen();
    for (start, end, expected) in [
        ((2, 1), (2, 2), Order::Forward),
        ((2, 2), (2, 1), Order::Reverse),
        ((2, 1), (3, 1), Order::Forward),
        ((2, 1), (2, 1), Order::Forward),
        ((2, 1), (1, 1), Order::Reverse),
    ] {
        assert_eq!(
            selection(&screen, start, end, false).order(&screen),
            Some(expected)
        );
    }
}

#[test]
fn selection_order_rectangle() {
    let screen = screen();
    for (start, end, expected) in [
        ((1, 1), (2, 2), Order::Forward),
        ((2, 2), (1, 1), Order::Reverse),
        ((3, 1), (1, 3), Order::MirroredForward),
        ((1, 3), (3, 1), Order::MirroredReverse),
        ((1, 1), (3, 1), Order::Forward),
        ((3, 1), (1, 1), Order::Reverse),
        ((2, 1), (2, 3), Order::Forward),
        ((2, 3), (2, 1), Order::Reverse),
        ((1, 1), (1, 1), Order::Forward),
    ] {
        assert_eq!(
            selection(&screen, start, end, true).order(&screen),
            Some(expected)
        );
    }
}

#[test]
fn selection_top_left() {
    let screen = screen();
    for (start, end) in [
        ((1, 1), (3, 1)),
        ((3, 1), (1, 1)),
        ((3, 1), (1, 3)),
        ((1, 3), (3, 1)),
    ] {
        assert!(
            selection(&screen, start, end, true)
                .top_left(&screen)
                .unwrap()
                .eql(pin(&screen, 1, 1))
        );
    }
}

#[test]
fn selection_bottom_right() {
    let screen = screen();
    for (start, end, rectangle, expected) in [
        ((1, 1), (3, 1), false, (3, 1)),
        ((3, 1), (1, 1), false, (3, 1)),
        ((3, 1), (1, 3), true, (3, 3)),
        ((1, 3), (3, 1), true, (3, 3)),
    ] {
        assert!(
            selection(&screen, start, end, rectangle)
                .bottom_right(&screen)
                .unwrap()
                .eql(pin(&screen, expected.0, expected.1))
        );
    }
}

#[test]
fn selection_ordered() {
    let screen = screen();
    for (start, end, rectangle, tl, br) in [
        ((1, 1), (3, 1), false, (1, 1), (3, 1)),
        ((3, 1), (1, 1), false, (1, 1), (3, 1)),
        ((3, 1), (1, 3), true, (1, 1), (3, 3)),
        ((1, 3), (3, 1), true, (1, 1), (3, 3)),
    ] {
        let sel = selection(&screen, start, end, rectangle);
        for desired in [
            Order::Forward,
            Order::Reverse,
            Order::MirroredForward,
            Order::MirroredReverse,
        ] {
            let result = sel.ordered(&screen, desired).unwrap();
            let (first, last) = if desired == Order::Reverse {
                (br, tl)
            } else {
                (tl, br)
            };
            // Matching authored orientation retains the original endpoints.
            let (first, last) = if sel.order(&screen) == Some(desired) {
                (start, end)
            } else {
                (first, last)
            };
            assert!(result.start.eql(pin(&screen, first.0, first.1)));
            assert!(result.end.eql(pin(&screen, last.0, last.1)));
            assert_eq!(result.rectangle, rectangle);
        }
    }
}

#[test]
fn selection_contains() {
    let screen = screen();
    for (start, end) in [((5, 1), (3, 2)), ((3, 2), (5, 1))] {
        let sel = selection(&screen, start, end, false);
        for (x, y, expected) in [(6, 1, true), (1, 2, true), (1, 1, false), (5, 2, false)] {
            assert_eq!(sel.contains(&screen, pin(&screen, x, y)), expected);
        }
    }
    let sel = selection(&screen, (5, 1), (8, 1), false);
    for (x, expected) in [(6, true), (2, false), (9, false)] {
        assert_eq!(sel.contains(&screen, pin(&screen, x, 1)), expected);
    }
    let sel = selection(&screen, (5, 1), (3, 3), false);
    assert!(sel.contains(&screen, pin(&screen, 0, 2)));
    assert!(!sel.contains(&screen, pin(&screen, 6, 4)));
}

#[test]
fn selection_contains_rectangle() {
    let screen = screen();
    for (start, end) in [((3, 3), (7, 9)), ((7, 9), (3, 3))] {
        let sel = selection(&screen, start, end, true);
        for (x, y, expected) in [
            (5, 6, true),
            (3, 6, true),
            (7, 6, true),
            (5, 3, true),
            (5, 9, true),
            (5, 2, false),
            (5, 10, false),
            (2, 6, false),
            (8, 6, false),
            (8, 3, false),
            (2, 9, false),
        ] {
            assert_eq!(sel.contains(&screen, pin(&screen, x, y)), expected);
        }
    }
    let sel = selection(&screen, (5, 1), (10, 1), true);
    for (x, expected) in [(6, true), (2, false), (12, false)] {
        assert_eq!(sel.contains(&screen, pin(&screen, x, 1)), expected);
    }
}

#[test]
fn selection_contained_row() {
    let screen = screen();
    for (start, end, rectangle, rows) in [
        ((5, 1), (3, 3), false, [(5, 14), (0, 14), (0, 3)]),
        ((3, 1), (6, 3), true, [(3, 6), (3, 6), (3, 6)]),
    ] {
        let sel = selection(&screen, start, end, rectangle);
        assert!(sel.contained_row(&screen, pin(&screen, 1, 4)).is_none());
        for (y, (left, right)) in [1, 2, 3].into_iter().zip(rows) {
            let row = sel.contained_row(&screen, pin(&screen, 1, y)).unwrap();
            assert!(row.start.eql(pin(&screen, left, y)));
            assert!(row.end.eql(pin(&screen, right, y)));
            assert_eq!(row.rectangle, rectangle);
        }
    }
    let sel = selection(&screen, (2, 1), (6, 1), false);
    assert!(sel.contained_row(&screen, pin(&screen, 1, 0)).is_none());
    assert!(sel.contained_row(&screen, pin(&screen, 1, 2)).is_none());
    let row = sel.contained_row(&screen, pin(&screen, 1, 1)).unwrap();
    assert!(row.start.eql(sel.start));
    assert!(row.end.eql(sel.end));
}

#[test]
fn selection_rectangle_corners_clamp_mixed_width_pages() {
    let mut screen = screen();
    screen.pages.split(pin(&screen, 0, 1)).unwrap();
    let narrow = pin(&screen, 0, 1).node;
    screen.pages.page_mut(narrow).unwrap().size.cols = 2;
    let sel = selection(&screen, (3, 0), (1, 1), true);
    assert_eq!(sel.order(&screen), Some(Order::MirroredForward));
    let corner = sel.bottom_right(&screen).unwrap();
    assert!(corner.eql(pin(&screen, 1, 1)));
    assert!(corner.row_and_cell(&screen.pages).is_some());
    let reverse = Selection::init(sel.end, sel.start, true);
    let corner = reverse.bottom_right(&screen).unwrap();
    assert!(corner.eql(pin(&screen, 1, 1)));
    assert!(corner.row_and_cell(&screen.pages).is_some());
}

#[test]
fn selection_contained_row_clamps_mixed_width_pages() {
    let mut screen = screen();
    screen.pages.split(pin(&screen, 0, 2)).unwrap();
    screen.pages.split(pin(&screen, 0, 1)).unwrap();
    let narrow = pin(&screen, 0, 1).node;
    screen.pages.page_mut(narrow).unwrap().size.cols = 2;
    for (start, end, rectangle, left) in [((1, 0), (1, 2), false, 0), ((1, 0), (3, 2), true, 1)] {
        let sel = selection(&screen, start, end, rectangle);
        let row = sel.contained_row(&screen, pin(&screen, 0, 1)).unwrap();
        assert!(row.start.eql(pin(&screen, left, 1)));
        assert!(row.end.eql(pin(&screen, 1, 1)));
        assert!(row.end.row_and_cell(&screen.pages).is_some());
    }
}

#[test]
fn selection_rejects_out_of_bounds_pins() {
    let screen = screen();
    let valid = pin(&screen, 0, 0);
    let invalid = Pin { x: 15, ..valid };
    let sel = Selection::init(valid, invalid, false);
    assert_eq!(sel.order(&screen), None);
    assert!(sel.top_left(&screen).is_none());
    assert!(sel.bottom_right(&screen).is_none());
    assert!(sel.ordered(&screen, Order::Forward).is_none());
    assert!(!sel.contains(&screen, valid));
    assert!(sel.contained_row(&screen, valid).is_none());
    let sel = Selection::init(valid, valid, false);
    assert!(!sel.contains(&screen, invalid));
    assert!(sel.contained_row(&screen, invalid).is_none());
}
