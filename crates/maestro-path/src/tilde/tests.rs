//! Flavor-independent witnesses of the `~` parse and of the concatenating expansion.
use super::{Tilde, expand_tilde_with_flavor, parse};
use std::io;

/// Expands `path` in one flavor over a fixed home.
fn expand(path: &str, windows: bool, home: &str) -> String {
    expand_tilde_with_flavor::<io::Error>(path, windows, || Ok(home.to_owned())).unwrap()
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
        ("~", false, Tilde::Home),
        ("~/", false, Tilde::Below("")),
        ("~/x", false, Tilde::Below("x")),
        ("~//x", false, Tilde::Below("/x")),
        ("~/../x", false, Tilde::Below("../x")),
        ("~x", false, Tilde::Other),
        ("~user/x", false, Tilde::Other),
        (r"~\x", false, Tilde::Other),
        ("./~", false, Tilde::Other),
        ("a~/x", false, Tilde::Other),
        ("", false, Tilde::Other),
        ("~", true, Tilde::Home),
        (r"~\x", true, Tilde::Below("x")),
        ("~/x", true, Tilde::Below("x")),
        (r"~\\x", true, Tilde::Below(r"\x")),
        (r"~x\y", true, Tilde::Other),
    ];
    for (written, windows, expected) in rows {
        assert_eq!(parse(written, windows), expected, "{written:?} {windows}");
    }
}

/// The concatenating expansion appends the rest as written; the home contributes its own
/// text, so an empty home leaves a rooted result and a home ending in a separator gains none.
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
        ("~/x", "/h/", "/h/x"),
        ("~/x", "/", "/x"),
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
        assert_eq!(expand(written, false, home), expected, "posix {written:?}");
    }
}

/// On Windows `~\x` expands, either separator joins natively and empty components drop.
#[test]
fn windows_concatenation_joins_natively() {
    let rows = [
        ("~", r"C:\h", r"C:\h"),
        (r"~\", r"C:\h", r"C:\h"),
        ("~/", r"C:\h", r"C:\h"),
        (r"~\x", r"C:\h", r"C:\h\x"),
        ("~/x", r"C:\h", r"C:\h\x"),
        (r"~/a\b/c", r"C:\h", r"C:\h\a\b\c"),
        (r"~\\x//y", r"C:\h", r"C:\h\x\y"),
        ("~/x", r"C:\h\\", r"C:\h\\x"),
        ("~/x", "C:/h/", "C:/h/x"),
        (r"~\..\x", r"C:\h", r"C:\h\..\x"),
        ("~/x", "", r"\x"),
        (r"~x", r"C:\h", r"~x"),
        (r"~x\y", r"C:\h", r"~x\y"),
    ];
    for (written, home, expected) in rows {
        assert_eq!(expand(written, true, home), expected, "win32 {written:?}");
    }
}

/// Only `~` and `~` plus a separator read the home directory; the lookup's error comes back.
#[test]
fn home_is_read_only_for_the_expanded_forms() {
    for windows in [false, true] {
        for written in ["", "x", "~x", "~user/x", "a~", "./~", "/~"] {
            assert_eq!(
                expand_tilde_with_flavor(written, windows, unread).unwrap(),
                written,
                "{written:?}"
            );
        }
        for written in ["~", "~/", "~/x"] {
            let error = expand_tilde_with_flavor(written, windows, unread).unwrap_err();
            assert_eq!(error.to_string(), "the home directory was read");
        }
    }
}
