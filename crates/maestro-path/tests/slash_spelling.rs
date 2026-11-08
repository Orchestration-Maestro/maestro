//! One slash-separated spelling for comparisons, on every host.

use maestro_path::slashed;

#[test]
fn the_windows_flavor_writes_every_separator_as_a_forward_slash() {
    for (path, expected) in [
        (r"D:\work\notes", "D:/work/notes"),
        (r"\\server\share\work", "//server/share/work"),
        (r"D:\work\notes\", "D:/work/notes/"),
        ("D:/work/notes", "D:/work/notes"),
    ] {
        assert_eq!(slashed(path, true), expected, "{path}");
    }
}

#[test]
fn the_posix_flavor_keeps_a_backslash_as_a_file_character() {
    for path in ["/work/notes", r"/work\notes", r"D:\work\notes", ""] {
        assert_eq!(slashed(path, false), path);
    }
}
