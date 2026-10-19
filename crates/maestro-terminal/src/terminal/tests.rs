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
