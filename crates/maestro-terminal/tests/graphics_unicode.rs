//! Placeholder runs decoded from styled terminal cells.

use maestro_terminal::{Mode, Terminal, kitty::graphics::unicode::placement_iterator, point::Tag};

#[test]
fn placeholders_continue_implicit_columns_and_split_explicit_gaps() {
    let mut terminal = Terminal::new(10, 5, 100_000).unwrap();
    terminal.modes.set(Mode::GraphemeCluster, true);
    terminal
        .print_string("ABC\u{10eeee}\u{305}\u{305}\u{10eeee}\u{10eeee}\u{305}\u{310}Z")
        .unwrap();
    let pages = &terminal.screens.active().pages;
    let mut iter = placement_iterator(pages.get_top_left(Tag::Viewport), None);
    let first = iter.next(pages).unwrap();
    assert_eq!(
        (first.pin.x, first.row, first.col, first.width),
        (3, 0, 0, 2)
    );
    let second = iter.next(pages).unwrap();
    assert_eq!((second.pin.x, second.col, second.width), (5, 3, 1));
    assert!(iter.next(pages).is_none());
}

/// Print a fixture and collect every placeholder run.
fn runs(text: &str) -> Vec<maestro_terminal::kitty::graphics::unicode::Placement> {
    let mut terminal = Terminal::new(10, 5, 100_000).unwrap();
    terminal.modes.set(Mode::GraphemeCluster, true);
    terminal.print_string(text).unwrap();
    let pages = &terminal.screens.active().pages;
    let mut iter = placement_iterator(pages.get_top_left(Tag::Viewport), None);
    std::iter::from_fn(|| iter.next(pages)).collect()
}

#[test]
fn placeholder_runs_support_explicit_implicit_and_missing_coordinates() {
    for text in [
        "\u{10eeee}\u{305}\u{305}\u{10eeee}\u{305}\u{30d}\u{10eeee}\u{305}\u{30e}",
        "\u{10eeee}\u{305}\u{305}\u{10eeee}\u{305}\u{10eeee}\u{305}",
        "\u{10eeee}\u{10eeee}\u{10eeee}",
    ] {
        let placements = runs(text);
        assert_eq!(placements.len(), 1);
        let p = placements[0];
        assert_eq!(
            (p.image_id, p.placement_id, p.row, p.col, p.width, p.height),
            (0, 0, 0, 0, 3, 1)
        );
    }
    assert!(runs("hello\r\nworld\r\n1\r\n2").is_empty());
    let placements = runs("\u{10eeee}\u{305}\u{305}ABC");
    assert_eq!(placements.len(), 1);
    assert_eq!(placements[0].width, 1);
}

#[test]
fn placeholder_colors_encode_image_high_bits_and_placement_identity() {
    use maestro_terminal::{color::Rgb, style::Color};
    for (fg, underline, suffix, image_id, placement_id) in [
        (Color::Palette(42), Color::None, "\u{305}\u{305}", 42, 0),
        (
            Color::Palette(42),
            Color::None,
            "\u{305}\u{305}\u{30e}",
            33_554_474,
            0,
        ),
        (
            Color::Palette(42),
            Color::Palette(21),
            "\u{305}\u{305}",
            42,
            21,
        ),
        (
            Color::Rgb(Rgb { r: 1, g: 2, b: 3 }),
            Color::Rgb(Rgb { r: 4, g: 5, b: 6 }),
            "\u{305}\u{305}",
            0x0001_0203,
            0x0004_0506,
        ),
    ] {
        let mut terminal = Terminal::new(5, 5, 0).unwrap();
        terminal.screens.active_mut().cursor.style.fg_color = fg;
        terminal.screens.active_mut().cursor.style.underline_color = underline;
        terminal.screens.active_mut().manual_style_update().unwrap();
        terminal
            .print_string(&format!("\u{10eeee}{suffix}"))
            .unwrap();
        let pages = &terminal.screens.active().pages;
        let mut iter = placement_iterator(pages.get_top_left(Tag::Viewport), None);
        let p = iter.next(pages).unwrap();
        assert_eq!(
            (p.image_id, p.placement_id, p.row, p.col),
            (image_id, placement_id, 0, 0)
        );
        assert!(iter.next(pages).is_none());
    }
}

#[test]
fn placeholder_diacritics_decode_high_indices_and_ignore_unknown_or_excess_values() {
    let placements = runs("\u{10eeee}\u{483}\u{1d242}\u{1d242}\u{305}");
    assert_eq!(
        (placements[0].row, placements[0].col, placements[0].image_id),
        (30, 294, 0)
    );
    let placements = runs("\u{10eeee}\u{301}\u{301}");
    assert_eq!((placements[0].row, placements[0].col), (0, 0));
}

#[test]
fn placeholder_runs_remain_row_local_and_respect_the_inclusive_row_limit() {
    let mut terminal = Terminal::new(5, 5, 0).unwrap();
    terminal.print_string("\u{10eeee}\r\n\u{10eeee}").unwrap();
    let pages = &terminal.screens.active().pages;
    let origin = pages.get_top_left(Tag::Viewport);
    let mut iter = placement_iterator(origin, Some(origin));
    assert_eq!(iter.next(pages).unwrap().width, 1);
    assert!(iter.next(pages).is_none());
    let mut iter = placement_iterator(origin, None);
    assert_eq!(iter.next(pages).unwrap().pin.y, 0);
    assert_eq!(iter.next(pages).unwrap().pin.y, 1);
    assert!(iter.next(pages).is_none());
}

#[test]
fn placeholder_runs_inherit_high_bits_but_split_row_and_identity_changes() {
    let placements = runs(
        "\u{10eeee}\u{305}\u{305}\u{30e}\u{10eeee}\u{10eeee}\u{30d}\u{30e}\u{30e}\u{10eeee}\u{30d}\u{310}\u{30d}",
    );
    assert_eq!(placements.len(), 3);
    assert_eq!(
        (placements[0].image_id, placements[0].width),
        (0x0200_0000, 2)
    );
    assert_eq!((placements[1].row, placements[1].width), (1, 1));
    assert_eq!(placements[2].image_id, 0x0100_0000);
}

#[test]
fn placeholder_runs_split_on_foreground_or_underline_changes() {
    use maestro_terminal::sgr::Attribute;
    let mut terminal = Terminal::new(10, 5, 0).unwrap();
    for attribute in [
        Attribute::Palette256Fg(42),
        Attribute::Palette256Fg(43),
        Attribute::Palette256UnderlineColor(7),
    ] {
        terminal.set_attribute(attribute).unwrap();
        terminal.print_string("\u{10eeee}").unwrap();
    }
    let pages = &terminal.screens.active().pages;
    let mut iter = placement_iterator(pages.get_top_left(Tag::Viewport), None);
    for expected in [(42, 0), (43, 0), (43, 7)] {
        let run = iter.next(pages).unwrap();
        assert_eq!((run.image_id, run.placement_id), expected);
        assert_eq!(run.width, 1);
    }
    assert!(iter.next(pages).is_none());
}
