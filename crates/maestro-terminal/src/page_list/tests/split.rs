//! Page splitting preserves links, row values and tracked coordinates.

use super::reflow::flags;
use super::resize::{integrity, screen, text, write};
use super::*;
use crate::page::CellPosition;

/// Ten rows with their row number in the first cell.
fn numbered() -> PageList {
    let mut list = screen(10, 10);
    for y in 0..10 {
        write(
            &mut list,
            y,
            &char::from_u32(u32::try_from(y).unwrap())
                .unwrap()
                .to_string(),
        );
    }
    list
}

/// Split at an absolute retained row's first cell.
fn at(list: &mut PageList, y: usize) {
    list.split(list.pin_at_row(y, 0).unwrap()).unwrap();
}

#[test]
fn history_split_at_middle_row() {
    let mut list = numbered();
    let first = list.first;
    at(&mut list, 5);
    let second = list.nodes.get(first).unwrap().next.unwrap();
    assert_eq!(list.total_pages(), 2);
    assert_eq!(list.page(first).unwrap().size.rows, 5);
    assert_eq!(list.page(second).unwrap().size.rows, 5);
    for y in 0..5 {
        for (node, offset) in [(first, 0), (second, 5)] {
            assert_eq!(
                list.page(node).unwrap().get_cells(y)[0].codepoint(),
                char::from_u32(u32::from(y + offset)).unwrap()
            );
        }
    }
    integrity(&list);
}

#[test]
fn history_split_at_row_0_is_no_op() {
    let mut list = numbered();
    let original = list.first;
    at(&mut list, 0);
    assert_eq!((list.first, list.last), (original, original));
    assert_eq!(list.page(original).unwrap().size.rows, 10);
    for y in 0..10 {
        assert_eq!(
            list.pin_at_row(y, 0)
                .unwrap()
                .row_and_cell(&list)
                .unwrap()
                .1
                .codepoint(),
            char::from_u32(u32::try_from(y).unwrap()).unwrap()
        );
    }
    integrity(&list);
}

#[test]
fn history_split_at_last_row() {
    let mut list = numbered();
    at(&mut list, 9);
    assert_eq!(list.total_pages(), 2);
    assert_eq!(list.page(list.first).unwrap().size.rows, 9);
    assert_eq!(list.page(list.last).unwrap().size.rows, 1);
    assert_eq!(
        list.page(list.last).unwrap().get_cells(0)[0].codepoint(),
        '\t'
    );
    assert!(text(&list, 8).starts_with('\u{8}'));
    integrity(&list);
}

#[test]
fn history_split_single_row_page_returns_out_of_space() {
    let mut list = screen(10, 1);
    let pin = list.get_top_left(crate::point::Tag::Active);
    assert_eq!(list.split(pin), Err(SplitError::OutOfSpace));
    assert_eq!((list.total_pages(), list.total_rows()), (1, 1));
    integrity(&list);
}

/// Assert the retained slot's coordinates and garbage flag.
fn retained(list: &PageList, handle: TrackedPin, expected: Pin) {
    let pin = *list.tracked_pin(handle).unwrap();
    assert_eq!(pin, expected);
    assert!(!pin.garbage);
}

#[test]
fn history_split_moves_tracked_pins() {
    let mut list = numbered();
    let pin = list.track_pin(list.pin_at_row(7, 3).unwrap()).unwrap();
    at(&mut list, 5);
    retained(
        &list,
        pin,
        Pin {
            node: list.last,
            x: 3,
            y: 2,
            garbage: false,
        },
    );
    assert!(text(&list, 7).starts_with('\u{7}'));
    integrity(&list);
}

#[test]
fn history_split_tracked_pin_before_split_point_unchanged() {
    let mut list = numbered();
    let original = list.pin_at_row(2, 5).unwrap();
    let pin = list.track_pin(original).unwrap();
    at(&mut list, 5);
    retained(&list, pin, original);
    assert!(text(&list, 2).starts_with('\u{2}'));
    integrity(&list);
}

#[test]
fn history_split_tracked_pin_at_split_point_moves_to_new_page() {
    let mut list = numbered();
    let pin = list.track_pin(list.pin_at_row(5, 4).unwrap()).unwrap();
    at(&mut list, 5);
    retained(
        &list,
        pin,
        Pin {
            node: list.last,
            x: 4,
            y: 0,
            garbage: false,
        },
    );
    assert!(text(&list, 5).starts_with('\u{5}'));
    integrity(&list);
}

#[test]
fn history_split_multiple_tracked_pins_across_regions() {
    let mut list = numbered();
    let pins = [(1, 0), (5, 2), (7, 3), (9, 8)]
        .map(|(y, x)| list.track_pin(list.pin_at_row(y, x).unwrap()).unwrap());
    at(&mut list, 5);
    for (handle, (moved, x, y)) in
        pins.into_iter()
            .zip([(false, 0, 1), (true, 2, 0), (true, 3, 2), (true, 8, 4)])
    {
        retained(
            &list,
            handle,
            Pin {
                node: if moved { list.last } else { list.first },
                x,
                y,
                garbage: false,
            },
        );
    }
    integrity(&list);
}

#[test]
fn history_split_tracked_viewport_pin_in_split_region_moves_correctly() {
    let mut list = numbered();
    *list.tracked_pin_mut(list.viewport_pin).unwrap() = list.pin_at_row(7, 6).unwrap();
    at(&mut list, 5);
    retained(
        &list,
        list.viewport_pin,
        Pin {
            node: list.last,
            x: 6,
            y: 2,
            garbage: false,
        },
    );
    integrity(&list);
}

#[test]
fn history_split_middle_page_preserves_linked_list_order() {
    let mut list = screen(10, 12);
    let first = list.first;
    at(&mut list, 4);
    let middle = list.last;
    assert_eq!(list.page(first).unwrap().size.rows, 4);
    assert_eq!(list.page(middle).unwrap().size.rows, 8);
    at(&mut list, 8);
    let tail = list.last;
    assert_eq!(list.first, first);
    assert_eq!(list.nodes.get(first).unwrap().next, Some(middle));
    assert_eq!(list.nodes.get(middle).unwrap().next, Some(tail));
    for key in [first, middle, tail] {
        assert_eq!(list.page(key).unwrap().size.rows, 4);
    }
    integrity(&list);
}

#[test]
fn history_split_last_page_makes_new_page_the_last() {
    let mut list = numbered();
    at(&mut list, 5);
    let previous = list.last;
    assert_eq!(list.page(previous).unwrap().size.rows, 5);
    at(&mut list, 7);
    assert_ne!(list.last, previous);
    assert_eq!(list.nodes.get(list.last).unwrap().prev, Some(previous));
    assert!(list.nodes.get(list.last).unwrap().next.is_none());
    assert_eq!(list.page(previous).unwrap().size.rows, 2);
    assert_eq!(list.page(list.last).unwrap().size.rows, 3);
    integrity(&list);
}

#[test]
fn history_split_first_page_keeps_original_as_first() {
    let mut list = numbered();
    let original = list.first;
    at(&mut list, 5);
    let second = list.last;
    at(&mut list, 2);
    assert_eq!(list.first, original);
    assert!(list.nodes.get(original).unwrap().prev.is_none());
    let inserted = list.nodes.get(original).unwrap().next.unwrap();
    assert_ne!(inserted, second);
    assert_eq!(list.nodes.get(inserted).unwrap().next, Some(second));
    assert_eq!(list.page(original).unwrap().size.rows, 2);
    assert_eq!(list.page(inserted).unwrap().size.rows, 3);
    assert_eq!(list.page(second).unwrap().size.rows, 5);
    integrity(&list);
}

#[test]
fn history_split_preserves_wrap_flags() {
    let mut list = screen(10, 10);
    let page = list.page_mut(list.first).unwrap();
    page.get_row_mut(5).set_wrap(true);
    page.get_row_mut(6).set_wrap_continuation(true);
    page.get_row_mut(7).set_wrap(true);
    page.get_row_mut(7).set_wrap_continuation(true);
    at(&mut list, 5);
    assert_eq!(flags(&list, 5), (true, false));
    assert_eq!(flags(&list, 6), (false, true));
    assert_eq!(flags(&list, 7), (true, true));
    integrity(&list);
}

#[test]
fn history_split_preserves_styled_cells() {
    let mut list = screen(10, 10);
    let mut bold = crate::Style::default();
    bold.flags.set_bold(true);
    let page = list.page_mut(list.first).unwrap();
    for y in 5..8 {
        let id = page.styles.add(bold).unwrap();
        page.get_cells_mut(y)[0] = crate::page::cell::Cell::init('S');
        page.get_cells_mut(y)[0].set_style_id(id);
        page.get_row_mut(y).set_styled(true);
    }
    at(&mut list, 5);
    assert_eq!(list.page(list.first).unwrap().styles.count(), 0);
    assert_eq!(list.page(list.last).unwrap().styles.count(), 1);
    for y in 5..8 {
        let pin = list.pin_at_row(y, 0).unwrap();
        let (row, cell) = pin.row_and_cell(&list).unwrap();
        assert_eq!(cell.codepoint(), 'S');
        assert_ne!(cell.style_id(), 0);
        assert!(pin.style(&list, *cell).unwrap().flags.bold());
        assert!(row.styled());
    }
    integrity(&list);
}

#[test]
fn history_split_preserves_grapheme_clusters() {
    let mut list = screen(10, 10);
    write(&mut list, 6, "👨");
    list.page_mut(list.first)
        .unwrap()
        .set_graphemes(CellPosition { x: 0, y: 6 }, &['\u{200d}', '👩'])
        .unwrap();
    at(&mut list, 5);
    assert_eq!(list.page(list.first).unwrap().grapheme_count(), 0);
    assert_eq!(list.page(list.last).unwrap().grapheme_count(), 1);
    let pin = list.pin_at_row(6, 0).unwrap();
    assert_eq!(pin.row_and_cell(&list).unwrap().1.codepoint(), '👨');
    assert!(pin.row_and_cell(&list).unwrap().0.grapheme());
    assert_eq!(
        pin.grapheme(&list, CellPosition { x: pin.x, y: pin.y }),
        Some(&['\u{200d}', '👩'][..])
    );
    integrity(&list);
}

#[test]
fn history_split_preserves_hyperlinks() {
    let mut list = screen(10, 10);
    write(&mut list, 7, "L");
    let page = list.page_mut(list.first).unwrap();
    let id = page
        .insert_hyperlink(crate::hyperlink::Hyperlink {
            id: crate::hyperlink::HyperlinkId::Implicit(0),
            uri: b"https://example.com".to_vec(),
        })
        .unwrap();
    page.set_hyperlink(CellPosition { x: 0, y: 7 }, id).unwrap();
    at(&mut list, 5);
    assert_eq!(list.page(list.first).unwrap().hyperlink_set.count(), 0);
    assert_eq!(list.page(list.last).unwrap().hyperlink_set.count(), 1);
    let pin = list.pin_at_row(7, 0).unwrap();
    assert_eq!(pin.row_and_cell(&list).unwrap().1.codepoint(), 'L');
    assert!(pin.row_and_cell(&list).unwrap().1.hyperlink());
    assert_eq!(pin.hyperlink_uri(&list), Some(&b"https://example.com"[..]));
    integrity(&list);
}
