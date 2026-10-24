//! Image-enabled terminal shared by the unit and integration test programs.
#![cfg(test)]

/// An enabled terminal with a 10x5 grid of 8x16-pixel cells.
pub(crate) fn kitty_terminal() -> maestro_terminal::Terminal {
    let mut terminal = maestro_terminal::Terminal::new(10, 5, 100_000).unwrap();
    terminal.enable_kitty_graphics();
    terminal.resize(10, 5, 8, 16).unwrap();
    terminal
}
