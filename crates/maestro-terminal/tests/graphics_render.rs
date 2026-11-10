//! Physical image geometry through retained screen positions.

use maestro_terminal::{
    Terminal,
    kitty::graphics::{
        bitmap::Image,
        images::{Location, Placement},
    },
};

#[test]
fn render_information_combines_clamped_source_and_viewport_geometry() {
    let mut terminal = Terminal::new(5, 3, 100_000).unwrap();
    let pages = &mut terminal.screens.active_mut().pages;
    let origin = pages.get_top_left(maestro_terminal::point::Tag::Viewport);
    let pin = pages.track_pin(origin).unwrap();
    let placement = Placement {
        location: Location::Pin(pin),
        source_x: 8,
        source_y: 19,
        source_width: 30,
        source_height: 50,
        columns: 2,
        rows: 3,
        ..Placement::default()
    };
    let image = Image {
        width: 10,
        height: 20,
        ..Image::default()
    };
    let info = placement.render_info(&image, pages, (4, 8));
    assert_eq!(
        (
            info.pixel_width,
            info.pixel_height,
            info.grid_cols,
            info.grid_rows
        ),
        (8, 24, 2, 3)
    );
    assert_eq!(
        (
            info.source_x,
            info.source_y,
            info.source_width,
            info.source_height
        ),
        (8, 19, 2, 1)
    );
    assert_eq!(
        (info.viewport_col, info.viewport_row, info.viewport_visible),
        (0, 0, true)
    );
}

#[test]
fn viewport_geometry_handles_hidden_and_partly_visible_images() {
    for (start_row, rows, newlines, expected_row, visible) in [
        (0, 1, 0, 0, true),
        (0, 1, 10, -6, false),
        (0, 4, 6, -2, true),
        (3, 4, 0, 3, true),
        (0, 10, 7, -3, true),
    ] {
        let mut terminal = Terminal::new(80, 5, 100_000).unwrap();
        terminal.enable_kitty_graphics();
        terminal.set_cursor_pos(start_row + 1, 1);
        let image = Image {
            width: 10,
            height: 20,
            ..Image::default()
        };
        let pages = &mut terminal.screens.active_mut().pages;
        let origin = pages
            .pin(maestro_terminal::Point::Active(
                maestro_terminal::Coordinate {
                    x: 0,
                    y: u32::try_from(start_row).unwrap(),
                },
            ))
            .unwrap();
        let pin = pages.track_pin(origin).unwrap();
        let placement = Placement {
            location: Location::Pin(pin),
            columns: 1,
            rows,
            ..Placement::default()
        };
        for _ in 0..newlines {
            terminal.linefeed().unwrap();
        }
        let pages = &terminal.screens.active().pages;
        assert_eq!(
            placement.viewport_pos(&image, pages, (10, 20)),
            (0, expected_row, visible)
        );
        assert_eq!(
            placement
                .render_info(&image, pages, (10, 20))
                .viewport_visible,
            visible
        );
    }
}

#[test]
fn source_rectangles_default_and_clamp_at_each_image_boundary() {
    let image = Image {
        width: 10,
        height: 20,
        ..Image::default()
    };
    assert_eq!(Placement::default().source_rect(&image), (0, 0, 10, 20));
    for (x, y, w, h, expected) in [
        (2, 3, 4, 5, (2, 3, 4, 5)),
        (2, 3, 0, 0, (2, 3, 8, 17)),
        (99, 99, 100, 100, (10, 20, 0, 0)),
    ] {
        let p = Placement {
            source_x: x,
            source_y: y,
            source_width: w,
            source_height: h,
            ..Placement::default()
        };
        assert_eq!(p.source_rect(&image), expected);
    }
    let terminal = Terminal::new(5, 3, 0).unwrap();
    assert!(
        !Placement::default()
            .render_info(&image, &terminal.screens.active().pages, (4, 8))
            .viewport_visible
    );
}
