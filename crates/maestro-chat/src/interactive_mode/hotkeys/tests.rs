//! Portable platform guidance through the keyboard-help formatter.
use super::editing_keys;

#[test]
fn hotkeys_newline_guidance_covers_supported_hosts() {
    let ordinary = editing_keys(false);
    let windows = editing_keys(true);
    let row = "| `Shift+Enter` | New line |";
    let windows_row = "| `Shift+Enter` | New line (Ctrl+Enter on Windows Terminal) |";
    assert_eq!(
        ordinary.lines().find(|line| line.contains("New line")),
        Some(row)
    );
    assert_eq!(
        windows.lines().find(|line| line.contains("New line")),
        Some(windows_row)
    );
    assert_eq!(windows, ordinary.replace(row, windows_row));
}
