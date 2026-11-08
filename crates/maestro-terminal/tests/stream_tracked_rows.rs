//! Retained row handles follow byte-driven scroll and screen lifetimes.

use maestro_terminal::{Coordinate, Point, Terminal, page_list::TrackedPin, screens::Key};

fn track(t: &mut Terminal) -> TrackedPin {
    let pages = &mut t.screens.active_mut().pages;
    let pin = pages.pin(Point::Active(Coordinate { x: 0, y: 0 })).unwrap();
    pages.track_pin(pin).unwrap()
}

#[test]
fn byte_tracks_tracked_grid_ref_snapshots_after_terminal_scroll() {
    let mut t = Terminal::new(5, 2, 10_000).unwrap();
    t.write(b"A");
    let tracked = track(&mut t);
    t.write(b"\r\nB\r\nC");
    let pages = &t.screens.active().pages;
    let pin = pages.tracked_pin(tracked).unwrap();
    assert!(!pin.garbage);
    assert_eq!(pin.row_and_cell(pages).unwrap().1.codepoint(), 'A');
}

#[test]
fn byte_tracks_tracked_grid_ref_reports_no_value_after_reset() {
    let mut t = Terminal::new(5, 2, 10_000).unwrap();
    t.write(b"A");
    let tracked = track(&mut t);
    t.full_reset();
    let pages = &t.screens.active().pages;
    assert!(pages.tracked_pin(tracked).unwrap().garbage);
}

#[test]
fn byte_tracks_tracked_grid_ref_reports_no_value_after_alternate_screen_reset() {
    let mut t = Terminal::new(5, 2, 10_000).unwrap();
    t.write(b"\x1b[?1049hA");
    let generation = t.screens.generation(Key::Alternate);
    let tracked = track(&mut t);
    t.write(b"\x1bc");
    assert_eq!(t.active_screen(), Key::Primary);
    assert!(t.screens.get(Key::Alternate).is_none());
    assert_ne!(t.screens.generation(Key::Alternate), generation);
    t.write(b"\x1b[?1049h");
    assert!(t.screens.active().pages.tracked_pin(tracked).is_none());
    let fresh = track(&mut t);
    assert!(!t.screens.active().pages.tracked_pin(fresh).unwrap().garbage);
}
