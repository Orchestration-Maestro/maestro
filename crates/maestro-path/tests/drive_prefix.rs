//! The drive prefix a Windows path opens with, on every host.

use maestro_path::win32;

#[test]
fn a_drive_prefix_is_a_letter_and_its_colon() {
    for path in [r"C:\work", r"d:\data", "Z:", "c:notes", "C:/work"] {
        assert!(win32::has_drive_prefix(path), "{path}");
    }
}

#[test]
fn a_path_without_a_drive_prefix_has_none() {
    for path in [
        "",
        "C",
        ":",
        "1:x",
        r"\C:\work",
        "CC:",
        r"\\server\share",
        r"\\?\C:\work",
    ] {
        assert!(!win32::has_drive_prefix(path), "{path}");
    }
}
