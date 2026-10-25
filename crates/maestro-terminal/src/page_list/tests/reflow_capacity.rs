//! Real managed-budget exhaustion while joining rows from different pages.

use super::resize::{integrity, nz, screen};
use super::*;
use crate::page::{CellPosition, cell::Cell as PackedCell};

/// Fill the first page and add one row in the next page without pruning history.
fn two_pages(cols: u16) -> PageList {
    let mut list = screen(cols, 10);
    assert_eq!(list.total_pages(), 1);
    let rows = usize::from(list.page(list.first).unwrap().capacity.rows);
    while list.total_rows() <= rows {
        if list.total_rows() == rows {
            assert_eq!(list.total_pages(), 1);
        }
        list.grow();
    }
    assert_eq!(list.total_pages(), 2);
    assert_ne!(list.first, list.last);
    assert_eq!(list.nodes.get(list.first).unwrap().next, Some(list.last));
    list
}

/// Retain the two source cells that will join at a page boundary.
fn boundary_pins(list: &mut PageList) -> [TrackedPin; 2] {
    let y = list.page(list.first).unwrap().size.rows - 1;
    let first = Pin {
        node: list.first,
        x: list.cols() - 1,
        y,
        garbage: false,
    };
    let second = Pin {
        node: list.last,
        x: 0,
        y: 0,
        garbage: false,
    };
    [
        list.track_pin(first).unwrap(),
        list.track_pin(second).unwrap(),
    ]
}

#[test]
fn history_resize_reflow_exceeds_grapheme_memory_forcing_capacity_increase() {
    let mut list = two_pages(2);
    let pins = boundary_pins(&mut list);
    let bytes = list.page(list.first).unwrap().capacity.grapheme_bytes;
    let suffix = vec!['a'; usize::try_from((bytes - 1) / 4).unwrap()];
    for pin in pins {
        let pin = *list.tracked_pin(pin).unwrap();
        let page = list.page_mut(pin.node).unwrap();
        page.get_cells_mut(pin.y)[usize::from(pin.x)] = PackedCell::init('X');
        page.get_row_mut(pin.y).set_wrap(true);
        page.set_graphemes(CellPosition { x: pin.x, y: pin.y }, &suffix)
            .unwrap();
    }
    list.resize(Resize {
        cols: Some(nz(3)),
        ..Resize::default()
    })
    .unwrap();
    for handle in pins {
        let pin = *list.tracked_pin(handle).unwrap();
        let page = list.page(pin.node).unwrap();
        assert_eq!(page.get_cells(pin.y)[usize::from(pin.x)].codepoint(), 'X');
        assert_eq!(
            page.lookup_grapheme(CellPosition { x: pin.x, y: pin.y })
                .unwrap(),
            suffix
        );
        assert!(page.capacity.grapheme_bytes > bytes);
    }
    integrity(&list);
}

#[test]
fn history_resize_reflow_exceeds_hyperlink_memory_forcing_capacity_increase() {
    let mut list = two_pages(2);
    let pins = boundary_pins(&mut list);
    let bytes = list.page(list.first).unwrap().capacity.string_bytes;
    let uri = vec![b'a'; usize::try_from(bytes - 1).unwrap()];
    for (index, handle) in pins.into_iter().enumerate() {
        let pin = *list.tracked_pin(handle).unwrap();
        let page = list.page_mut(pin.node).unwrap();
        page.get_cells_mut(pin.y)[usize::from(pin.x)] = PackedCell::init('X');
        page.get_row_mut(pin.y).set_wrap(index == 0);
        page.get_row_mut(pin.y).set_wrap_continuation(index == 1);
        let id = page
            .insert_hyperlink(crate::hyperlink::Hyperlink {
                id: crate::hyperlink::HyperlinkId::Implicit(u32::try_from(index).unwrap()),
                uri: uri.clone(),
            })
            .unwrap();
        page.set_hyperlink(CellPosition { x: pin.x, y: pin.y }, id)
            .unwrap();
        assert_eq!(
            page.insert_hyperlink(crate::hyperlink::Hyperlink {
                id: crate::hyperlink::HyperlinkId::Implicit(u32::try_from(index + 1).unwrap()),
                uri: vec![b'A'; 26],
            }),
            Err(crate::page::StorageError::StringsOutOfMemory)
        );
    }
    list.resize(Resize {
        cols: Some(nz(3)),
        ..Resize::default()
    })
    .unwrap();
    for (index, handle) in pins.into_iter().enumerate() {
        let pin = *list.tracked_pin(handle).unwrap();
        let page = list.page(pin.node).unwrap();
        let id = page
            .lookup_hyperlink(CellPosition { x: pin.x, y: pin.y })
            .unwrap();
        let link = page.hyperlink_set.get(id);
        assert_eq!(link.uri, uri);
        assert_eq!(
            link.id,
            crate::hyperlink::HyperlinkId::Implicit(u32::try_from(index).unwrap())
        );
        assert!(page.capacity.string_bytes > bytes);
    }
    integrity(&list);
}

#[test]
fn history_resize_reflow_exceeds_style_memory_forcing_capacity_increase() {
    let cols = STD_CAPACITY.styles - 1;
    let mut list = two_pages(cols);
    let original = list.page(list.first).unwrap().capacity.styles;
    let pins = boundary_pins(&mut list);
    for (index, handle) in pins.into_iter().enumerate() {
        let pin = *list.tracked_pin(handle).unwrap();
        let page = list.page_mut(pin.node).unwrap();
        for x in 0..cols {
            let color = crate::style::Color::Rgb(crate::color::Rgb {
                r: u8::try_from(x).unwrap(),
                g: 0,
                b: 0,
            });
            let style = if index == 0 {
                crate::Style {
                    bg_color: color,
                    ..crate::Style::default()
                }
            } else {
                crate::Style {
                    fg_color: color,
                    ..crate::Style::default()
                }
            };
            let Ok(id) = page.styles.add(style) else {
                break;
            };
            let cell = &mut page.get_cells_mut(pin.y)[usize::from(x)];
            *cell = PackedCell::init('X');
            cell.set_style_id(id);
        }
        page.get_row_mut(pin.y).set_styled(true);
        page.get_row_mut(pin.y).set_wrap(index == 0);
        page.get_row_mut(pin.y).set_wrap_continuation(index == 1);
    }
    list.resize(Resize {
        cols: Some(nz(cols * 2)),
        ..Resize::default()
    })
    .unwrap();
    let pin = *list.tracked_pin(pins[1]).unwrap();
    let page = list.page(pin.node).unwrap();
    assert!(page.capacity.styles > original);
    let left = page.get_cells(pin.y)[0];
    let right = page.get_cells(pin.y)[usize::from(cols)];
    assert_eq!(left.codepoint(), 'X');
    assert_eq!(right.codepoint(), 'X');
    assert_ne!(
        page.styles.get(left.style_id()),
        page.styles.get(right.style_id())
    );
    integrity(&list);
}
