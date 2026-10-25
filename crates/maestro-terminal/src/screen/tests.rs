//! Screen managed-state behavior tests.

mod attributes;
mod clearing;
mod cursor;
mod hyperlink;

use super::{Options, Screen};
use crate::{
    Page,
    page::cell::Content,
    page_list::{NodeId, Pin},
};
use std::num::NonZeroU16;

/// Construct a small screen with enough active storage for controlled fixtures.
fn screen(cols: u16, rows: u16) -> Screen {
    Screen::init(Options {
        cols: NonZeroU16::new(cols).unwrap(),
        rows: NonZeroU16::new(rows).unwrap(),
        max_scrollback: 100_000,
    })
}

/// Resolve the actual retained cursor, never a copied cell buffer.
fn pin(screen: &Screen) -> Pin {
    *screen.pages.tracked_pin(screen.cursor.page_pin).unwrap()
}

/// Resolve the cursor's real managed storage.
fn page(screen: &Screen) -> &Page {
    screen.pages.page(pin(screen).node).unwrap()
}

/// Populate an empty row with cursor styling and optional hyperlink ownership.
fn write_row(screen: &mut Screen, text: &str) {
    for (x, ch) in text.chars().enumerate() {
        let x = u16::try_from(x).unwrap();
        screen.cursor_horizontal_absolute(x);
        let pin = pin(screen);
        let style_id = screen.cursor.style_id;
        let page = screen.pages.page_mut(pin.node).unwrap();
        let cell = &mut page.get_cells_mut(pin.y)[usize::from(x)];
        assert_eq!(cell.style_id(), 0);
        cell.set_content(Content::Codepoint(ch));
        cell.set_style_id(style_id);
        if style_id != 0 {
            page.styles.use_multiple(style_id, 1);
            page.get_row_mut(pin.y).set_styled(true);
        }
        if screen.cursor.hyperlink_id != 0 {
            screen.cursor_set_hyperlink().unwrap();
        }
    }
}

/// Create a real active page boundary without using the later screen printer.
fn split_rows(screen: &mut Screen, y: u16) -> (NodeId, NodeId) {
    let first = pin(screen).node;
    screen.pages.split(Pin { y, ..pin(screen) }).unwrap();
    let second = Pin {
        y: 0,
        ..pin(screen)
    }
    .down(&screen.pages, usize::from(y))
    .unwrap()
    .node;
    (first, second)
}

/// Fill the native style owner with distinct values until admission refuses.
fn fill_styles(page: &mut Page) {
    for value in 1..=u16::MAX {
        let style = crate::style::Style {
            bg_color: crate::style::Color::Rgb(crate::color::Rgb {
                r: value.to_le_bytes()[0],
                g: value.to_le_bytes()[1],
                b: 1,
            }),
            ..crate::style::Style::default()
        };
        if page.styles.add(style).is_err() {
            return;
        }
    }
    panic!("style table did not reach its declared capacity");
}
