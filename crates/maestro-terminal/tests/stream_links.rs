//! Byte-created links use the retained cell URI owner.

use maestro_terminal::{Coordinate, Point, Terminal};

#[test]
fn byte_links_grid_ref_hyperlink_uri_no_hyperlink() {
    let mut t = Terminal::new(80, 24, 10_000).unwrap();
    t.write(b"hello");
    let pages = &t.screens.active().pages;
    let pin = pages.pin(Point::Active(Coordinate { x: 0, y: 0 })).unwrap();
    assert_eq!(pin.hyperlink_uri(pages), None);
}

#[test]
fn byte_links_grid_ref_hyperlink_uri_with_hyperlink() {
    let mut t = Terminal::new(80, 24, 10_000).unwrap();
    t.write(b"\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\plain");
    let pages = &t.screens.active().pages;
    for x in 0..4 {
        let pin = pages.pin(Point::Active(Coordinate { x, y: 0 })).unwrap();
        assert_eq!(
            pin.hyperlink_uri(pages),
            Some(b"https://example.com".as_slice())
        );
    }
    let pin = pages.pin(Point::Active(Coordinate { x: 4, y: 0 })).unwrap();
    assert_eq!(pin.hyperlink_uri(pages), None);
}
