//! Terminal tab destinations and line control.

use maestro_terminal::{Terminal, csi::TabClear};

#[test]
fn terminal_clear_absent_tab_does_not_create_a_destination() {
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    terminal.screens.active_mut().cursor_horizontal_absolute(3);
    terminal.tab_clear(TabClear::Current);
    terminal.screens.active_mut().cursor_horizontal_absolute(0);
    terminal.horizontal_tab();
    assert_eq!(terminal.screens.active().cursor.x, 8);
}

#[test]
fn line_control_horizontal_tabs_back() {
    let mut terminal = Terminal::new(20, 5, 0).unwrap();
    terminal.screens.active_mut().cursor_horizontal_absolute(19);
    terminal.horizontal_tab_back();
    assert_eq!(terminal.screens.active().cursor.x, 16);
    terminal.horizontal_tab_back();
    assert_eq!(terminal.screens.active().cursor.x, 8);
    terminal.horizontal_tab_back();
    assert_eq!(terminal.screens.active().cursor.x, 0);
    terminal.horizontal_tab_back();
    assert_eq!(terminal.screens.active().cursor.x, 0);
}

#[test]
fn line_control_tabclear_single() {
    let mut terminal = Terminal::new(30, 5, 0).unwrap();
    terminal.horizontal_tab();
    terminal.tab_clear(TabClear::Current);
    let screen = terminal.screens.active_mut();
    let pin = screen
        .pages
        .pin(maestro_terminal::Point::Active(
            maestro_terminal::Coordinate { x: 0, y: 0 },
        ))
        .unwrap();
    assert_eq!(pin.is_dirty(&screen.pages), Some(false));
    screen.cursor_absolute(0, 0);
    terminal.horizontal_tab();
    assert_eq!(terminal.screens.active().cursor.x, 16);
}

#[test]
fn line_control_tabclear_all() {
    let mut terminal = Terminal::new(30, 5, 0).unwrap();
    terminal.tab_clear(TabClear::All);
    let screen = terminal.screens.active_mut();
    let pin = screen
        .pages
        .pin(maestro_terminal::Point::Active(
            maestro_terminal::Coordinate { x: 0, y: 0 },
        ))
        .unwrap();
    assert_eq!(pin.is_dirty(&screen.pages), Some(false));
    screen.cursor_absolute(0, 0);
    terminal.horizontal_tab();
    assert_eq!(terminal.screens.active().cursor.x, 29);
}
