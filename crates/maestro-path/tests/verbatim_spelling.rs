//! The ordinary spelling of verbatim Windows paths, on every host.

use maestro_path::win32;

#[test]
fn verbatim_and_ordinary_spellings_of_one_windows_path_agree() {
    for (verbatim, ordinary) in [
        (r"\\?\D:\work\notes.md", r"D:\work\notes.md"),
        (r"\\?\c:\", r"c:\"),
        (r"\\?\D:", "D:"),
        (r"\\?\c:", "c:"),
        (r"\\?\UNC\server\share\work", r"\\server\share\work"),
        (r"\\?\UNC\server\share", r"\\server\share"),
        (r"\\?\unc\server\share\skills", r"\\server\share\skills"),
    ] {
        assert_eq!(win32::ordinary(verbatim), ordinary, "{verbatim}");
        assert_eq!(win32::ordinary(ordinary), ordinary, "{ordinary}");
    }
}

#[test]
fn verbatim_paths_without_a_drive_prefix_or_share_keep_their_spelling() {
    for path in [
        r"\\?\Volume{0b1c2d3e-0000-0000-0000-000000000000}\work",
        r"\\?\C:skills",
        r"\\?\UNC\",
        r"\\?\UNCserver",
        r"\\.\D:\work",
        r"//?/D:/work",
        r"D:\work",
        "D:work",
        "",
    ] {
        assert_eq!(win32::ordinary(path), path);
    }
}
