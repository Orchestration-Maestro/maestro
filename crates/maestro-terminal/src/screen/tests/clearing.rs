//! Clearing, managed neighbors and wide-cell boundary behavior.

use super::{page, pin, screen, write_row};
use crate::{Cell, Coordinate, Point, page::cell::Wide, sgr::Attribute, style::Color};

#[test]
fn screen_clear_rows_active_one_line() {
    let mut s = screen(20, 3);
    write_row(&mut s, "hello, world");
    s.clear_rows(Point::Active(Coordinate::default()), None, false);
    assert!(page(&s).get_row(0).dirty());
    assert!(page(&s).get_cells(0).iter().all(|cell| cell.is_zero()));
}

#[test]
fn screen_clear_rows_active_multi_line() {
    let mut s = screen(20, 3);
    write_row(&mut s, "hello");
    s.cursor_absolute(0, 1);
    write_row(&mut s, "world");
    s.clear_rows(Point::Active(Coordinate::default()), None, false);
    for y in 0..3 {
        assert!(page(&s).get_row(y).dirty());
        assert!(page(&s).get_cells(y).iter().all(|cell| cell.is_zero()));
    }
}

#[test]
fn screen_clear_rows_active_styled_line() {
    let mut s = screen(20, 3);
    s.set_attribute(Attribute::Bold).unwrap();
    write_row(&mut s, "hello world");
    s.set_attribute(Attribute::Unset).unwrap();
    assert_eq!(page(&s).styles.count(), 1);
    s.clear_rows(Point::Active(Coordinate::default()), None, false);
    assert_eq!(page(&s).styles.count(), 0);
    assert!(page(&s).get_cells(0).iter().all(|cell| cell.is_zero()));
}

#[test]
fn screen_clear_cells_empty_range() {
    let mut s = screen(5, 1);
    write_row(&mut s, "hello");
    let before = page(&s).get_cells(0).to_vec();
    let row = *page(&s).get_row(0);
    s.clear_cells(pin(&s).node, 0, 0..0);
    assert_eq!(page(&s).get_cells(0), before);
    assert_eq!(*page(&s).get_row(0), row);
}

#[test]
fn screen_clear_rows_uses_stored_page_width() {
    let mut s = screen(4, 1);
    write_row(&mut s, "AB");
    let node = pin(&s).node;
    s.pages.page_mut(node).unwrap().size.cols = 2;
    s.cursor_horizontal_absolute(0);
    s.clear_rows(Point::Screen(Coordinate::default()), None, false);
    assert_eq!(page(&s).get_cells(0).len(), 2);
    assert!(page(&s).get_cells(0).iter().all(|cell| cell.is_zero()));
}

#[test]
fn screen_clear_rows_protected() {
    let mut s = screen(8, 2);
    for y in 0..2 {
        s.cursor_absolute(0, y);
        write_row(&mut s, "ABCDEFGH");
        let page = s.pages.page_mut(pin(&s).node).unwrap();
        for x in 0..8 {
            page.get_cells_mut(y)[x].set_protected((x / 2 + usize::from(y)) % 2 == 0);
        }
        page.get_row_mut(y).set_wrap(true);
    }
    s.clear_rows(Point::Active(Coordinate::default()), None, true);
    for y in 0..2 {
        for (x, cell) in page(&s).get_cells(y).iter().enumerate() {
            assert_eq!(cell.is_zero(), (x / 2 + usize::from(y)) % 2 != 0);
        }
        assert!(page(&s).get_row(y).wrap());
        assert!(page(&s).get_row(y).dirty());
    }
}

#[test]
fn screen_blank_cell_keeps_only_background() {
    use crate::{color::Rgb, page::cell::Content};
    for (attribute, expected) in [
        (Attribute::Unset, Content::Codepoint('\0')),
        (Attribute::Palette256Fg(2), Content::Codepoint('\0')),
        (Attribute::Palette256Bg(3), Content::BgColorPalette(3)),
        (
            Attribute::DirectColorBg(Rgb { r: 1, g: 2, b: 3 }),
            Content::BgColorRgb(Rgb { r: 1, g: 2, b: 3 }),
        ),
    ] {
        let mut s = screen(5, 1);
        s.set_attribute(Attribute::Bold).unwrap();
        s.set_attribute(attribute).unwrap();
        s.cursor.protected = true;
        s.start_hyperlink(b"link", None).unwrap();
        write_row(&mut s, "A");
        s.pages
            .page_mut(pin(&s).node)
            .unwrap()
            .append_grapheme(crate::page::CellPosition { x: 0, y: 0 }, '\u{301}')
            .unwrap();
        let blank = s.blank_cell();
        assert_eq!(blank.content(), expected);
        assert_eq!(blank.style_id(), 0);
        assert!(!blank.protected());
        assert!(!blank.hyperlink());
        assert_eq!(blank.wide(), Wide::Narrow);
        assert!(!blank.has_text());
    }
}

#[test]
fn screen_protected_clear_preserves_managed_neighbours() {
    let mut s = screen(5, 1);
    s.set_attribute(Attribute::Bold).unwrap();
    s.start_hyperlink(b"https://example.test", None).unwrap();
    write_row(&mut s, "ABCDE");
    let p = pin(&s);
    let style = s.cursor.style_id;
    let link = s.cursor.hyperlink_id;
    {
        let page = s.pages.page_mut(p.node).unwrap();
        page.get_cells_mut(0)[2].set_protected(true);
        for x in 1..4 {
            page.append_grapheme(crate::page::CellPosition { x, y: 0 }, '\u{301}')
                .unwrap();
        }
    }
    s.set_attribute(Attribute::Palette256Bg(3)).unwrap();
    s.clear_unprotected_cells(p.node, 0, 1..4);
    let page = page(&s);
    assert_eq!(page.styles.ref_count(style), 3);
    assert_eq!(page.hyperlink_set.ref_count(link), 4);
    assert_protected_neighbours(&s, style);
    assert_eq!(
        page.lookup_grapheme(crate::page::CellPosition { x: 2, y: 0 }),
        Some(['\u{301}'].as_slice())
    );
    for x in [1, 3] {
        assert!(
            page.lookup_grapheme(crate::page::CellPosition { x, y: 0 })
                .is_none()
        );
    }
    assert!(page.get_row(0).grapheme());
    assert!(page.get_row(0).styled());
    assert!(page.get_row(0).hyperlink());
    assert_eq!(s.cursor.style.bg_color, Color::Palette(3));
}

/// Verify retained neighbor styles and links alongside background-only cleared cells.
fn assert_protected_neighbours(s: &super::Screen, style: crate::style::Id) {
    for x in [0, 2, 4] {
        assert_eq!(page(s).get_cells(0)[x].style_id(), style);
        assert!(page(s).get_cells(0)[x].hyperlink());
    }
    for x in [1, 3] {
        assert_eq!(page(s).get_cells(0)[x], s.blank_cell());
    }
}

#[test]
fn screen_wrap_reset_clears_only_wrap_and_spacer_state() {
    let mut s = screen(4, 2);
    write_row(&mut s, "ABCD");
    let p = pin(&s);
    {
        let page = s.pages.page_mut(p.node).unwrap();
        page.get_row_mut(0).set_wrap(true);
        page.get_row_mut(1).set_wrap_continuation(true);
        page.get_cells_mut(0)[3].set_wide(Wide::SpacerHead);
    }
    s.cursor.pending_wrap = true;
    s.cursor_reset_wrap();
    assert!(!s.cursor.pending_wrap);
    assert!(!page(&s).get_row(0).wrap());
    assert!(!page(&s).get_row(1).wrap_continuation());
    assert_eq!(page(&s).get_cells(0)[0], Cell::init('A'));
    assert!(page(&s).get_cells(0)[3].is_zero());
    s.pages.page_mut(p.node).unwrap().get_cells_mut(0)[3].set_wide(Wide::SpacerHead);
    s.cursor_reset_wrap();
    assert_eq!(page(&s).get_cells(0)[3].wide(), Wide::SpacerHead);
}

#[test]
fn screen_wide_boundary_split_clears_only_disconnected_halves() {
    for (boundary, cross_page) in (0..=4).flat_map(|b| [(b, false), (b, true)]) {
        let mut s = screen(4, 3);
        let p = pin(&s);
        {
            let page = s.pages.page_mut(p.node).unwrap();
            page.get_row_mut(0).set_wrap(true);
            page.get_cells_mut(0)[3].set_wide(Wide::SpacerHead);
            page.get_row_mut(1).set_wrap_continuation(true);
            page.get_row_mut(1).set_wrap(true);
            page.get_cells_mut(1)[0] = Cell::init('界');
            page.get_cells_mut(1)[0].set_wide(Wide::Wide);
            page.get_cells_mut(1)[1].set_wide(Wide::SpacerTail);
            page.get_cells_mut(1)[2] = Cell::init('X');
            page.get_cells_mut(1)[3].set_wide(Wide::SpacerHead);
        }
        if cross_page {
            super::split_rows(&mut s, 1);
        }
        s.cursor_absolute(0, 1);
        s.split_cell_boundary(boundary);
        let previous = s.pages.page(p.node).unwrap();
        let current = page(&s);
        let y = u16::from(!cross_page);
        assert_eq!(previous.get_cells(0)[3].is_zero(), boundary <= 1);
        assert_eq!(current.get_cells(y)[0].is_zero(), boundary == 1);
        assert_eq!(current.get_cells(y)[1].is_zero(), boundary == 1);
        assert_eq!(current.get_cells(y)[2], Cell::init('X'));
        assert_eq!(current.get_cells(y)[3].is_zero(), boundary == 4);
        assert!(current.get_row(y).wrap());
        assert!(current.get_row(y).wrap_continuation());
    }
}

#[test]
fn screen_protected_clear_empty_full_and_all_protected() {
    for protected in [false, true] {
        let mut control = screen(5, 1);
        control.set_attribute(Attribute::Bold).unwrap();
        control.start_hyperlink(b"control", None).unwrap();
        write_row(&mut control, "ABCDE");
        let p = pin(&control);
        let style = control.cursor.style_id;
        let link = control.cursor.hyperlink_id;
        for cell in control.pages.page_mut(p.node).unwrap().get_cells_mut(0) {
            cell.set_protected(protected);
        }
        let before = page(&control).get_cells(0).to_vec();
        control.clear_unprotected_cells(p.node, 0, 0..0);
        assert_eq!(page(&control).get_cells(0), before);
        control.clear_unprotected_cells(p.node, 0, 0..5);
        let expected_owners = if protected { 6 } else { 1 };
        assert_eq!(page(&control).styles.ref_count(style), expected_owners);
        assert_eq!(
            page(&control).hyperlink_set.ref_count(link),
            expected_owners
        );
        if protected {
            assert_eq!(page(&control).get_cells(0), before);
        } else {
            assert!(page(&control).get_cells(0).iter().all(|c| c.is_zero()));
        }
    }
}

#[test]
fn screen_detached_style_keeps_background_blank() {
    let mut s = screen(5, 1);
    assert!(s.blank_cell().is_zero());
    s.set_attribute(Attribute::Palette256Bg(7)).unwrap();
    let mut expected = Cell::default();
    expected.set_content(crate::page::cell::Content::BgColorPalette(7));
    assert_eq!(s.blank_cell(), expected);
    s.with_cursor_detached(crate::screen::cursor::LinkRestore::Preserve, |screen| {
        assert_eq!(screen.cursor.style_id, 0);
        assert_eq!(screen.blank_cell(), expected);
    });
    assert_eq!(s.blank_cell(), expected);
}

#[test]
fn screen_prompt_redraw_clears_with_default_background() {
    let mut s = screen(5, 2);
    write_row(&mut s, "A");
    let p = pin(&s);
    s.pages
        .page_mut(p.node)
        .unwrap()
        .get_row_mut(0)
        .set_semantic_prompt(crate::page::row::SemanticPrompt::Prompt);
    s.cursor.semantic_content = crate::page::cell::SemanticContent::Prompt;
    s.set_attribute(Attribute::Palette256Bg(7)).unwrap();
    s.resize(crate::screen::Resize {
        cols: std::num::NonZeroU16::new(6).unwrap(),
        rows: std::num::NonZeroU16::new(2).unwrap(),
        reflow: true,
        prompt_redraw: crate::osc::semantic_prompt::Redraw::True,
    })
    .unwrap();
    assert!(page(&s).get_cells(0).iter().all(|cell| cell.is_zero()));
    assert_eq!(s.cursor.style.bg_color, Color::Palette(7));
}
