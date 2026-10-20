//! Placeholder runs decoded from styled terminal cells.

use maestro_terminal::{
    Mode, Terminal,
    kitty::graphics::{
        image::Image,
        storage::{ImageStorage, Placement},
        unicode::{self, Error, placement_iterator},
    },
    point::Tag,
};

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

#[test]
fn placeholder_render_fits_and_centers_the_image_across_rows() {
    let (mut terminal, mut storage, run) = render_fixture();
    let screen = terminal.screens.active_mut();
    storage
        .add_placement(
            &mut screen.pages,
            1,
            7,
            Placement {
                columns: 4,
                rows: 2,
                ..Placement::default()
            },
        )
        .unwrap();
    let image = storage.image_by_id(1).unwrap();
    let render = run.render_placement(&storage, image, (36, 80)).unwrap();
    assert_eq!(
        (
            render.offset_x,
            render.offset_y,
            render.source_x,
            render.source_y
        ),
        (0, 36, 0, 0)
    );
    assert_eq!(
        (
            render.source_width,
            render.source_height,
            render.dest_width,
            render.dest_height
        ),
        (500, 153, 144, 44)
    );
    let wildcard = unicode::Placement {
        placement_id: 0,
        ..run
    }
    .render_placement(&storage, image, (36, 80))
    .unwrap();
    assert_eq!((wildcard.dest_width, wildcard.dest_height), (144, 44));
    let render = unicode::Placement { row: 1, ..run }
        .render_placement(&storage, image, (36, 80))
        .unwrap();
    assert_eq!(
        (
            render.offset_y,
            render.source_y,
            render.source_height,
            render.dest_height
        ),
        (0, 153, 153, 44)
    );
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
fn placeholder_render_handles_blank_padding_single_cell_and_prototype_errors() {
    let (mut terminal, mut storage, mut run) = render_fixture();
    let screen = terminal.screens.active_mut();
    assert_eq!(
        run.render_placement(&storage, storage.image_by_id(1).unwrap(), (36, 80))
            .unwrap_err(),
        Error::PlacementMissingPlacement
    );
    for (cols, rows, row, offset, source_y, source_height, width, height) in [
        (2, 2, 0, 58, 0, 153, 72, 22),
        (2, 2, 1, 0, 153, 153, 72, 22),
        (1, 1, 0, 29, 0, 306, 36, 22),
    ] {
        storage
            .add_placement(
                &mut screen.pages,
                1,
                7,
                Placement {
                    columns: cols,
                    rows,
                    ..Placement::default()
                },
            )
            .unwrap();
        run.row = row;
        let render = run
            .render_placement(&storage, storage.image_by_id(1).unwrap(), (36, 80))
            .unwrap();
        assert_eq!(
            (
                render.offset_y,
                render.source_y,
                render.source_height,
                render.dest_width,
                render.dest_height
            ),
            (offset, source_y, source_height, width, height)
        );
    }
}

#[test]
fn placeholder_grid_uses_native_size_and_rejects_unrepresentable_counts() {
    let (mut terminal, mut storage, run) = render_fixture();
    let screen = terminal.screens.active_mut();
    storage
        .add_placement(
            &mut screen.pages,
            1,
            7,
            Placement {
                columns: 0,
                rows: 0,
                ..Placement::default()
            },
        )
        .unwrap();
    assert!(
        run.render_placement(&storage, storage.image_by_id(1).unwrap(), (36, 80))
            .is_ok()
    );
    storage
        .add_placement(
            &mut screen.pages,
            1,
            7,
            Placement {
                columns: 65_536,
                rows: 1,
                ..Placement::default()
            },
        )
        .unwrap();
    assert_eq!(
        run.render_placement(&storage, storage.image_by_id(1).unwrap(), (36, 80))
            .unwrap_err(),
        Error::PlacementGridOutOfBounds
    );
}

/// An uploaded image and its unresolved placeholder fragment.
fn render_fixture() -> (Terminal, ImageStorage, unicode::Placement) {
    let mut terminal = Terminal::new(10, 5, 100_000).unwrap();
    let image = Image {
        id: 1,
        width: 500,
        height: 306,
        ..Image::default()
    };
    let mut storage = ImageStorage::default();
    storage
        .add_image(&mut terminal.screens.active_mut().pages, image)
        .unwrap();
    let run = unicode::Placement {
        pin: terminal.screens.active().pages.get_top_left(Tag::Viewport),
        image_id: 1,
        placement_id: 7,
        col: 0,
        row: 0,
        width: 4,
        height: 1,
    };
    (terminal, storage, run)
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
fn placeholder_render_horizontally_centers_tall_images() {
    let (mut terminal, mut storage, run) = render_fixture();
    let pages = &mut terminal.screens.active_mut().pages;
    storage
        .add_image(
            pages,
            Image {
                id: 1,
                width: 306,
                height: 500,
                ..Image::default()
            },
        )
        .unwrap();
    storage
        .add_placement(
            pages,
            1,
            7,
            Placement {
                columns: 2,
                rows: 4,
                ..Placement::default()
            },
        )
        .unwrap();
    let render = unicode::Placement {
        width: 1,
        height: 4,
        ..run
    }
    .render_placement(&storage, storage.image_by_id(1).unwrap(), (80, 36))
    .unwrap();
    assert_eq!(
        (
            render.offset_x,
            render.source_x,
            render.source_width,
            render.dest_width,
            render.dest_height
        ),
        (36, 0, 153, 44, 144)
    );
}

#[test]
fn placeholder_render_omits_fragments_entirely_in_blank_padding() {
    let (mut terminal, mut storage, run) = render_fixture();
    let pages = &mut terminal.screens.active_mut().pages;
    storage
        .add_placement(
            pages,
            1,
            7,
            Placement {
                columns: 2,
                rows: 20,
                ..Placement::default()
            },
        )
        .unwrap();
    let render = run
        .render_placement(&storage, storage.image_by_id(1).unwrap(), (36, 80))
        .unwrap();
    assert_eq!(
        (
            render.source_width,
            render.source_height,
            render.dest_width,
            render.dest_height
        ),
        (0, 0, 0, 0)
    );
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
