//! Screen suffix growth through the retained cursor owner.

use crate::{Cell, Screen, page::CellPosition, screen::Options, sgr::Attribute};

#[test]
fn screen_suffix_growth_reloads_target_without_losing_cursor_owners() {
    for target_x in [0, 1, 2] {
        let mut screen = Screen::init(Options::default());
        screen.cursor_horizontal_absolute(1);
        screen.set_attribute(Attribute::Bold).unwrap();
        screen
            .start_hyperlink(b"https://example.test/cluster", Some(b"cluster"))
            .unwrap();
        let handle = screen.cursor.page_pin;
        let initial = *screen.pages.tracked_pin(handle).unwrap();
        let position = CellPosition {
            x: target_x,
            y: initial.y,
        };
        screen
            .pages
            .page_mut(initial.node)
            .unwrap()
            .get_cells_mut(initial.y)[usize::from(target_x)] = Cell::init('o');
        let expected = screen.cursor.clone();
        let capacity = screen
            .pages
            .page(initial.node)
            .unwrap()
            .capacity
            .grapheme_bytes;
        for _ in 0..3000 {
            screen.append_grapheme(target_x, '\u{301}').unwrap();
        }
        let pin = *screen.pages.tracked_pin(handle).unwrap();
        assert_ne!(pin.node, initial.node);
        assert!(screen.pages.page(initial.node).is_none());
        assert_eq!(screen.cursor.page_pin, handle);
        assert_eq!((screen.cursor.x, screen.cursor.y), (1, 0));
        let page = screen.pages.page(pin.node).unwrap();
        assert!(page.capacity.grapheme_bytes > capacity);
        assert_eq!(
            page.lookup_grapheme(position).unwrap(),
            vec!['\u{301}'; 3000]
        );
        assert_eq!(
            page.get_cells(position.y)[usize::from(target_x)].codepoint(),
            'o'
        );
        assert!(page.get_row(position.y).grapheme());
        assert_cursor_owners(&screen, &expected);
    }
}

/// Check concrete values and their single retained cursor reference after growth.
fn assert_cursor_owners(screen: &Screen, expected: &crate::screen::Cursor) {
    let pin = screen.pages.tracked_pin(screen.cursor.page_pin).unwrap();
    let page = screen.pages.page(pin.node).unwrap();
    assert_eq!(screen.cursor.style, expected.style);
    assert_eq!(*page.styles.get(screen.cursor.style_id), expected.style);
    assert_eq!(page.styles.ref_count(screen.cursor.style_id), 1);
    assert_eq!(screen.cursor.hyperlink, expected.hyperlink);
    assert_eq!(
        page.hyperlink_set.get(screen.cursor.hyperlink_id),
        expected.hyperlink.as_deref().unwrap()
    );
    assert_eq!(page.hyperlink_set.ref_count(screen.cursor.hyperlink_id), 1);
}
