//! Flavor-independent witnesses of the `~` parse and of the concatenating expansion.
use super::{Rest, Separators, Tilde, expand_tilde, parse, shorten_home_in};
use std::io;

/// Expands `path` over a fixed home.
fn expand(path: &str, home: &str) -> String {
    expand_tilde::<io::Error>(path, || Ok(home.to_owned())).unwrap()
}

/// A home lookup that must not run.
fn unread() -> io::Result<String> {
    Err(io::Error::other("the home directory was read"))
}

/// An exact `~` is the home, `~` plus a separator holds the rest, and every other spelling is
/// not a home form.
#[test]
fn parse_reads_three_forms() {
    let rows = [
        ("~", Separators::Slash, Tilde::Home),
        ("~/", Separators::Slash, Tilde::Below("")),
        ("~/x", Separators::Slash, Tilde::Below("x")),
        ("~//x", Separators::Slash, Tilde::Below("/x")),
        ("~/../x", Separators::Slash, Tilde::Below("../x")),
        ("~x", Separators::Slash, Tilde::Other),
        ("~user/x", Separators::Slash, Tilde::Other),
        (r"~\x", Separators::Slash, Tilde::Other),
        ("./~", Separators::Slash, Tilde::Other),
        ("a~/x", Separators::Slash, Tilde::Other),
        ("", Separators::Slash, Tilde::Other),
        ("~", Separators::Windows, Tilde::Home),
        (r"~\x", Separators::Windows, Tilde::Below("x")),
        ("~/x", Separators::Windows, Tilde::Below("x")),
        (r"~\\x", Separators::Windows, Tilde::Below(r"\x")),
        (r"~x\y", Separators::Windows, Tilde::Other),
    ];
    for (written, separators, expected) in rows {
        assert_eq!(
            parse(written, separators),
            expected,
            "{written:?} {separators:?}"
        );
    }
}

/// The concatenating expansion appends the rest as written; the home contributes its own
/// text, including a trailing separator, and an empty home leaves a rooted result.
#[test]
fn posix_concatenation_appends_the_rest_as_written() {
    let rows = [
        ("~", "/h", "/h"),
        ("~", "/h/.", "/h/."),
        ("~", "", ""),
        ("~/", "/h", "/h/"),
        ("~/", "", "/"),
        ("~/x", "", "/x"),
        ("~/x", "/h", "/h/x"),
        ("~/x", "/h/", "/h//x"),
        ("~/x", "/", "//x"),
        ("~/x", r"C:\h\", r"C:\h\/x"),
        ("~//x", "/h", "/h//x"),
        ("~/../x", "/h", "/h/../x"),
        ("~x", "/h", "~x"),
        ("~user/x", "/h", "~user/x"),
        (r"~\x", "/h", r"~\x"),
        ("./~", "/h", "./~"),
        ("a~/x", "/h", "a~/x"),
        ("", "/h", ""),
    ];
    for (written, home, expected) in rows {
        assert_eq!(expand(written, home), expected, "posix {written:?}");
    }
}

/// Windows tool and configuration paths keep forward-slash suffixes verbatim.
#[test]
fn windows_concatenation_appends_the_rest_as_written() {
    let rows = [
        ("~", r"C:\h", r"C:\h"),
        (r"~\", r"C:\h", r"~\"),
        ("~/", r"C:\h", r"C:\h/"),
        (r"~\x", r"C:\h", r"~\x"),
        ("~/x", r"C:\h", r"C:\h/x"),
        ("~/a//b", r"C:\h", r"C:\h/a//b"),
        (r"~/a\b/c", r"C:\h", r"C:\h/a\b/c"),
        ("~/x", r"C:\h\", r"C:\h\/x"),
        ("~/x", "C:/h/", "C:/h//x"),
        ("~/x", "", "/x"),
        (r"~x", r"C:\h", r"~x"),
    ];
    for (written, home, expected) in rows {
        assert_eq!(expand(written, home), expected, "win32 {written:?}");
    }
    for written in [r"~\x", r"~\"] {
        assert_eq!(expand_tilde(written, unread).unwrap(), written);
    }
}

/// Only `~` and `~` plus a separator read the home directory; the lookup's error comes back.
#[test]
fn home_is_read_only_for_the_expanded_forms() {
    for written in ["", "x", "~x", "~user/x", "a~", "./~", "/~", r"~\x", r"~\"] {
        assert_eq!(
            expand_tilde(written, unread).unwrap(),
            written,
            "{written:?}"
        );
    }
    for written in ["~", "~/", "~/x"] {
        let error = expand_tilde(written, unread).unwrap_err();
        assert_eq!(error.to_string(), "the home directory was read");
    }
}

/// Shortens `path` below `home` in one flavor under one rule for the part below.
fn shorten(path: &str, home: &str, windows: bool, rest: Rest) -> String {
    shorten_home_in(path, home, windows, rest)
}

/// Both rules leave a path as written when it is not below a usable home.
#[test]
fn shorten_home_keeps_paths_that_are_not_below_a_usable_home() {
    let rows = [
        ("/srv/u2/x", "/srv/u"),
        ("/srv/u-other/x", "/srv/u"),
        ("/srv/u2", "/srv/u"),
        ("/etc", "/"),
        ("/", "/"),
        ("/etc", ""),
        ("/srv/u/x", "u"),
        ("u/x", "/srv/u"),
        ("", "/srv/u"),
        ("/tmp/file", "/srv/u"),
    ];
    for rest in [Rest::AsWritten, Rest::Resolved] {
        for (path, home) in rows {
            assert_eq!(
                shorten(path, home, false, rest),
                path,
                "{path:?} below {home:?} as {rest:?}"
            );
        }
    }
}

/// The part below the home keeps its spelling: dots, doubled and trailing separators stay.
#[test]
fn shorten_home_keeps_the_part_below_the_home_as_written() {
    let rows = [
        ("/srv/u/x", "/srv/u", "~/x"),
        ("/srv/u", "/srv/u", "~"),
        ("/srv/u/", "/srv/u", "~/"),
        ("/srv/u/a/b", "/srv/u", "~/a/b"),
        ("/srv/u/x", "/srv/u/", "~/x"),
        ("/srv/u/x", "/srv/u//", "~/x"),
        ("/srv/u/./a/../x", "/srv/u", "~/./a/../x"),
        ("/srv/u//x", "/srv/u", "~//x"),
        ("/srv/u/../x", "/srv/u", "~/../x"),
        ("/srv/x/../u/a", "/srv/u", "/srv/x/../u/a"),
        (
            "/srv/\u{e9}l\u{e8}ve/\u{6587}",
            "/srv/\u{e9}l\u{e8}ve",
            "~/\u{6587}",
        ),
    ];
    for (path, home, expected) in rows {
        assert_eq!(
            shorten(path, home, false, Rest::AsWritten),
            expected,
            "{path:?} below {home:?}"
        );
    }
}

/// The normalizing rule folds `.` and `..` and drops doubled and trailing separators.
#[test]
fn shorten_home_normalized_folds_the_part_below_the_home() {
    let rows = [
        ("/srv/u/x", "/srv/u", "~/x"),
        ("/srv/u", "/srv/u", "~"),
        ("/srv/u/", "/srv/u", "~"),
        ("/srv/u/a/b", "/srv/u", "~/a/b"),
        ("/srv/u/x", "/srv/u/", "~/x"),
        ("/srv/u/./a/../x", "/srv/u", "~/x"),
        ("/srv/u//x", "/srv/u", "~/x"),
        ("/srv/x/../u/a", "/srv/u", "~/a"),
        ("/srv/u/../x", "/srv/u", "/srv/u/../x"),
        (
            "/srv/\u{e9}l\u{e8}ve/\u{6587}",
            "/srv/\u{e9}l\u{e8}ve",
            "~/\u{6587}",
        ),
    ];
    for (path, home, expected) in rows {
        assert_eq!(
            shorten(path, home, false, Rest::Resolved),
            expected,
            "{path:?} below {home:?}"
        );
    }
}

/// The Windows flavor of the as-written rule follows its separators and drive roots, and
/// compares the home's text exactly.
#[test]
fn shorten_home_follows_the_windows_flavor() {
    let rows = [
        (r"C:\Users\ada\x", r"C:\Users\ada", r"~\x"),
        (r"C:\Users\ada", r"C:\Users\ada", "~"),
        (r"C:\Users\ada\.\x", r"C:\Users\ada\", r"~\.\x"),
        (r"C:/Users/ada/x", r"C:\Users\ada", r"C:/Users/ada/x"),
        (r"C:\Users\ada\x", r"C:\Users\ada\\", r"~\x"),
        (r"C:\Users\ada2\x", r"C:\Users\ada", r"C:\Users\ada2\x"),
        (r"D:\Users\ada\x", r"C:\Users\ada", r"D:\Users\ada\x"),
        (r"C:\Users\ada\x", r"C:\", r"C:\Users\ada\x"),
        (r"/home/ada/x", r"/home/ada", r"~/x"),
    ];
    for (path, home, expected) in rows {
        assert_eq!(
            shorten(path, home, true, Rest::AsWritten),
            expected,
            "{path:?} below {home:?}"
        );
    }
}

/// The Windows flavor of the normalizing rule follows its separator, drive roots and letter case.
#[test]
fn shorten_home_normalized_follows_the_windows_flavor() {
    let rows = [
        (r"C:\Users\ada\x", r"C:\Users\ada", r"~\x"),
        (r"C:\Users\ada", r"C:\Users\ada", "~"),
        (r"c:\users\ADA\x", r"C:\Users\ada", r"~\x"),
        (r"C:/Users/ada/x", r"C:\Users\ada", r"~\x"),
        (r"C:\Users\ada\.\x", r"C:\Users\ada", r"~\x"),
        (r"C:\Users\ada2\x", r"C:\Users\ada", r"C:\Users\ada2\x"),
        (r"D:\Users\ada\x", r"C:\Users\ada", r"D:\Users\ada\x"),
        (r"C:\Users\ada\x", r"C:\", r"C:\Users\ada\x"),
    ];
    for (path, home, expected) in rows {
        assert_eq!(
            shorten(path, home, true, Rest::Resolved),
            expected,
            "{path:?} below {home:?}"
        );
    }
}

/// Native Windows paths accept either separator and retain the home text.
#[test]
fn native_windows_expansion_keeps_its_separator_rules() {
    let rows = [
        (r"~\", r"C:\h", Some(r"C:\h")),
        ("~/", r"C:\h", Some(r"C:\h")),
        (r"~\x", r"C:\h", Some(r"C:\h\x")),
        (r"~/a\b/c", r"C:\h", Some(r"C:\h\a\b\c")),
        (r"~\\x//y", r"C:\h", Some(r"C:\h\x\y")),
        (r"~\x", r"C:\h\\", Some(r"C:\h\\x")),
        (r"~\x", "C:/h/", Some("C:/h/x")),
        (r"~\..\x", r"C:\h", Some(r"C:\h\..\x")),
        (r"~\x", "", Some(r"\x")),
        (r"~x\y", r"C:\h", None),
    ];
    for (written, home, expected) in rows {
        let expanded = super::expand(
            written,
            Separators::Windows,
            || Ok::<_, io::Error>(home.to_owned()),
            String::push_str,
        )
        .unwrap();
        assert_eq!(expanded.as_deref(), expected, "{written:?} over {home:?}");
    }
    let error = super::expand(r"~\x", Separators::Windows, unread, String::push_str).unwrap_err();
    assert_eq!(error.to_string(), "the home directory was read");
}
