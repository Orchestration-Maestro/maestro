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
