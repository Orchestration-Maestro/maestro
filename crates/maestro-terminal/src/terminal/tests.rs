//! Terminal ownership checks requiring access to retained private storage.

use super::Terminal;

#[test]
fn terminal_tab_set_and_reset_keep_custom_positions() {
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    terminal.screens.active_mut().cursor_horizontal_absolute(3);
    terminal.tab_set();
    terminal.screens.active_mut().cursor_horizontal_absolute(77);
    terminal.tab_set();
    terminal.tabstops.resize(160);
    assert!(terminal.tabstops.get(3));
    assert!(terminal.tabstops.get(77));
    assert!(terminal.tabstops.get(72));
    assert!(!terminal.tabstops.get(80));
    assert!(!terminal.tabstops.get(88));
    terminal.tabstops.resize(16);
    terminal.tabstops.resize(160);
    assert!(terminal.tabstops.get(3));
    assert!(terminal.tabstops.get(77));
    assert!(!terminal.tabstops.get(88));
    terminal.tabstops.resize(80);
    terminal.tab_reset();
    assert!(!terminal.tabstops.get(3));
    assert!(!terminal.tabstops.get(77));
    assert!(terminal.tabstops.get(8));
    assert!(terminal.tabstops.get(72));
    terminal.tabstops.resize(160);
    assert!(!terminal.tabstops.get(80));
    assert!(!terminal.tabstops.get(88));
}

mod printing;

/// Measure swaps and suffix-flag refreshes, optionally repeating a whole-row scan.
fn character_delete_work(cols: u16, rescan: bool) -> usize {
    let mut terminal = Terminal::new(cols, 1, 0).unwrap();
    terminal
        .print_string(&"x".repeat(usize::from(cols)))
        .unwrap();
    terminal.set_cursor_pos(1, 1);
    let pin = *terminal
        .screens
        .active()
        .pages
        .tracked_pin(terminal.screens.active().cursor.page_pin)
        .unwrap();
    let page = terminal
        .screens
        .active_mut()
        .pages
        .page_mut(pin.node)
        .unwrap();
    page.row_shift_work = 0;
    if rescan {
        for _ in 1..cols {
            page.update_row_grapheme_flag(pin.y);
        }
    }
    terminal.delete_chars(1);
    let page = terminal.screens.active().pages.page(pin.node).unwrap();
    assert!(
        page.get_cells(pin.y)[..usize::from(cols - 1)]
            .iter()
            .all(|cell| cell.codepoint() == 'x')
    );
    assert!(page.get_cells(pin.y)[usize::from(cols - 1)].is_zero());
    page.row_shift_work
}

#[test]
fn character_deletion_visits_survivors_once() {
    let small = character_delete_work(8, false);
    let large = character_delete_work(16, false);
    assert_eq!((small, large), (15, 31));
    assert_eq!(large + 1, 2 * (small + 1));
    let rescanned_small = character_delete_work(8, true);
    let rescanned_large = character_delete_work(16, true);
    assert!(rescanned_small > small);
    assert!(rescanned_large > 2 * rescanned_small);
}

/// Count full-row transfer work independently of the vacated row's clearing.
fn full_row_shift_work(cols: u16, insert: bool) -> usize {
    let mut terminal = Terminal::new(cols, 3, 0).unwrap();
    terminal.print_string("a\r\nb\r\nc").unwrap();
    terminal.screens.active_mut().cursor_absolute(0, 0);
    let pin = terminal
        .screens
        .active()
        .pages
        .get_top_left(crate::point::Tag::Active);
    terminal
        .screens
        .active_mut()
        .pages
        .page_mut(pin.node)
        .unwrap()
        .row_shift_work = 0;
    if insert {
        terminal.insert_lines(1).unwrap();
    } else {
        terminal.delete_lines(1).unwrap();
    }
    let page = terminal.screens.active().pages.page(pin.node).unwrap();
    let expected = if insert {
        ['\0', 'a', 'b']
    } else {
        ['b', 'c', '\0']
    };
    for (y, expected) in expected.into_iter().enumerate() {
        assert_eq!(
            page.get_cells(u16::try_from(y).unwrap())[0].codepoint(),
            expected
        );
    }
    page.row_shift_work
}

#[test]
fn full_width_insert_work_is_constant_per_row() {
    assert_eq!(full_row_shift_work(8, true), full_row_shift_work(16, true));
}

#[test]
fn full_width_delete_work_is_constant_per_row() {
    assert_eq!(
        full_row_shift_work(8, false),
        full_row_shift_work(16, false)
    );
}

#[test]
fn character_erasure_releases_refcounted_styles() {
    use crate::sgr::Attribute;
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.screens
        .active_mut()
        .set_attribute(Attribute::Bold)
        .unwrap();
    t.print_string("AB").unwrap();
    t.screens
        .active_mut()
        .set_attribute(Attribute::Unset)
        .unwrap();
    t.print('C').unwrap();
    let pin = *t
        .screens
        .active()
        .pages
        .tracked_pin(t.screens.active().cursor.page_pin)
        .unwrap();
    assert_eq!(
        t.screens
            .active()
            .pages
            .page(pin.node)
            .unwrap()
            .styles
            .count(),
        1
    );
    t.set_cursor_pos(1, 1);
    t.erase_chars(2);
    assert_eq!(
        t.screens
            .active()
            .pages
            .page(pin.node)
            .unwrap()
            .styles
            .count(),
        0
    );
}

#[test]
fn alignment_visits_each_cell_once() {
    let small = alignment_work(8);
    let large = alignment_work(16);
    eprintln!(
        "alignment N8={} N16={}; repeated N8={} N16={}",
        small.0, large.0, small.1, large.1
    );
    assert_eq!(large.0, small.0 * 2);
    assert!(large.1 > small.1 * 3);
}

/// Count alignment writes and a repeated-prefix read control over the same cells.
fn alignment_work(cols: u16) -> (usize, usize) {
    let mut t = Terminal::new(cols, 4, 0).unwrap();
    let node = t
        .screens
        .active()
        .pages
        .tracked_pin(t.screens.active().cursor.page_pin)
        .unwrap()
        .node;
    t.screens
        .active_mut()
        .pages
        .page_mut(node)
        .unwrap()
        .row_shift_work = 0;
    t.decaln().unwrap();
    let page = t.screens.active().pages.page(node).unwrap();
    let mut repeated = 0;
    for y in 0..4 {
        for end in 0..usize::from(cols) {
            for cell in &page.get_cells(y)[..=end] {
                assert_eq!(cell.codepoint(), 'E');
                repeated += 1;
            }
        }
    }
    (page.row_shift_work, repeated)
}
