//! Logical byte budgets and visible managed values through repeated pruning.

use super::resize::{integrity, nz};
use super::*;
use crate::page::{CellPosition, cell::Cell as PackedCell};
use crate::point::Tag;
use crate::{Style, style::Color};

/// Controls at two and three standard pages, with full and high-water style reuse.
const CONTROLS: [(usize, u16); 4] = [(2, 0), (3, 0), (3, 96), (3, 104)];
/// Byte-exact link, including a non-UTF-8 byte and an embedded NUL.
const URI: &[u8] = &[0xff, 0, b'u'];

#[test]
fn history_budget_retains_at_least_recorded_rows() {
    for (pages, styles) in CONTROLS {
        replay(pages, styles, |list, appended| {
            assert!(list.charged_bytes() <= list.max_size());
            assert_eq!(list.max_size(), recorded_limit(pages));
            assert!(list.total_rows() >= recorded_rows(pages, styles, appended));
            integrity(list);
        });
    }
}

#[test]
fn history_growth_preserves_visible_content() {
    for (pages, styles) in CONTROLS {
        replay(pages, styles, visible);
    }
}

/// Reproduce every append checkpoint, including the initial visible screen.
fn replay(pages: usize, styles: u16, mut check: impl FnMut(&PageList, usize)) {
    let mut list = PageList::init(nz(430), nz(24), Some(pages * 401_408));
    prepare(&mut list, styles);
    for y in 0..24 {
        write(&mut list, y, usize::from(y));
    }
    check(&list, 0);
    let mut unequal = false;
    let mut pruned = false;
    for appended in 1..=1000 {
        let first = list.first;
        list.grow();
        pruned |= !list.node_is_valid(first);
        write(&mut list, 23, appended + 23);
        unequal |=
            list.page(list.first).unwrap().capacity != list.page(list.last).unwrap().capacity;
        check(&list, appended);
    }
    assert!(pruned);
    assert_eq!(unequal, styles == 104);
}

/// Whether this target sizes pages in 16 KiB memory pages, where every page holds more rows.
const LARGE_PAGES: bool = crate::page::layout::PAGE_SIZE == 16_384;

/// The recorded effective limit of a control's budget, which never falls below two
/// standard pages.
fn recorded_limit(pages: usize) -> u64 {
    if LARGE_PAGES && pages == 2 {
        819_200
    } else {
        u64::try_from(pages).unwrap() * 401_408
    }
}

/// Compact the recorded checkpoint rows without imposing exact native retention.
fn recorded_rows(pages: usize, styles: u16, appended: usize) -> usize {
    // With 16 KiB host pages a page holds 111 rows of 430 columns, and every control's budget
    // keeps two such pages.
    let (boundaries, page_rows): (&[usize], usize) = if LARGE_PAGES {
        (&[199, 310, 421, 532, 643, 754, 865, 976], 111)
    } else if pages == 2 {
        (&[193, 301, 409, 517, 625, 733, 841, 949], 108)
    } else if styles > 0 {
        (&[193, 409, 517, 625, 733, 841, 949], 108)
    } else {
        (&[301, 409, 517, 625, 733, 841, 949], 108)
    };
    24 + appended
        - boundaries
            .iter()
            .filter(|&&boundary| boundary <= appended)
            .count()
            * page_rows
}

/// Fill the style store, release interior slots, reinsert and finally release all.
fn prepare(list: &mut PageList, count: u16) {
    let original = list.page(list.first).unwrap().capacity.styles;
    for x in 0..count {
        add_style(list, x, 1);
    }
    let filled = list.page(list.first).unwrap().capacity.styles;
    assert_eq!(filled > original, count == 104);
    for x in (0..count).step_by(2) {
        list.page_mut(list.first).unwrap().clear_cells(0, x..x + 1);
    }
    for x in (0..count).step_by(2) {
        add_style(list, x, 3);
    }
    assert_eq!(list.page(list.first).unwrap().capacity.styles, filled);
    let page = list.page_mut(list.first).unwrap();
    page.clear_cells(0, 0..count);
    assert_eq!(page.styles.count(), 0);
}

/// Retry a real capacity refusal through the list's existing replacement owner.
fn add_style(list: &mut PageList, x: u16, green: u8) {
    let value = Style {
        bg_color: Color::Rgb(crate::color::Rgb {
            r: u8::try_from(x).unwrap(),
            g: green,
            b: green + 1,
        }),
        ..Style::default()
    };
    loop {
        let page = list.page_mut(list.first).unwrap();
        if let Ok(id) = page.styles.add(value) {
            page.get_cells_mut(0)[usize::from(x)] = PackedCell::init('P');
            page.get_cells_mut(0)[usize::from(x)].set_style_id(id);
            page.get_row_mut(0).set_styled(true);
            break;
        }
        list.increase_capacity(list.first, Some(IncreaseCapacity::Styles))
            .unwrap();
    }
}

/// Write a row marker with sparse managed values, staying within their own budgets.
fn write(list: &mut PageList, y: u16, sequence: usize) {
    let pin = list
        .pin(Point::Active(Coordinate {
            x: 0,
            y: u32::from(y),
        }))
        .unwrap();
    let page = list.page_mut(pin.node).unwrap();
    page.get_cells_mut(pin.y)[0] =
        PackedCell::init(char::from(b'A' + u8::try_from(sequence % 26).unwrap()));
    if !sequence.is_multiple_of(8) {
        return;
    }
    let id = page
        .styles
        .add(Style {
            bg_color: Color::Palette(7),
            ..Style::default()
        })
        .unwrap();
    page.get_cells_mut(pin.y)[1] = PackedCell::init('s');
    page.get_cells_mut(pin.y)[1].set_style_id(id);
    page.get_row_mut(pin.y).set_styled(true);
    page.get_cells_mut(pin.y)[2] = PackedCell::init('e');
    page.append_grapheme(CellPosition { x: 2, y: pin.y }, '\u{301}')
        .unwrap();
    page.get_cells_mut(pin.y)[3] = PackedCell::init('l');
    let link = page
        .insert_hyperlink(crate::hyperlink::Hyperlink {
            id: crate::hyperlink::HyperlinkId::Implicit(0),
            uri: URI.to_vec(),
        })
        .unwrap();
    page.set_hyperlink(CellPosition { x: 3, y: pin.y }, link)
        .unwrap();
}

/// Compare every addressable visible cell and resolved value, never storage IDs.
fn visible(list: &PageList, appended: usize) {
    let mut rows = list.row_iterator(
        Direction::RightDown,
        Point::Active(Coordinate::default()),
        None,
    );
    for y in 0..24 {
        let pin = rows.next(list).unwrap();
        let page = list.page(pin.node).unwrap();
        for (x, &cell) in page.get_cells(pin.y).iter().enumerate() {
            let managed = (appended + y).is_multiple_of(8);
            if x >= 4 || (!managed && x > 0) {
                assert_eq!(cell, PackedCell::default());
                continue;
            }
            let codepoint = match x {
                0 => char::from(b'A' + u8::try_from((appended + y) % 26).unwrap()),
                1 if managed => 's',
                2 if managed => 'e',
                3 if managed => 'l',
                _ => '\0',
            };
            assert_eq!(cell.codepoint(), codepoint);
            let at = Pin {
                x: u16::try_from(x).unwrap(),
                ..pin
            };
            visible_values(list, at, cell, managed);
        }
    }
    assert!(rows.next(list).is_none());
    assert_eq!(
        list.point_from_pin(Tag::Active, list.get_top_left(Tag::Active)),
        Some(Point::Active(Coordinate::default()))
    );
}

/// Resolve the managed fields of a visible cell through the actual owning page.
fn visible_values(list: &PageList, at: Pin, cell: PackedCell, managed: bool) {
    let position = CellPosition { x: at.x, y: at.y };
    let style = if managed && at.x == 1 {
        Style {
            bg_color: Color::Palette(7),
            ..Style::default()
        }
    } else {
        Style::default()
    };
    assert_eq!(at.style(list, cell), Some(style));
    assert_eq!(
        at.grapheme(list, position),
        (managed && at.x == 2).then_some(&['\u{301}'][..])
    );
    assert_eq!(
        at.hyperlink_uri(list),
        (managed && at.x == 3).then_some(URI)
    );
}
