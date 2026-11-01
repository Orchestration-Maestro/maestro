//! Flavor-independent witnesses of the `~` parse and of the concatenating expansion.
use super::{Separators, Tilde, expand_tilde, parse};
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
            parse(written, matches!(separators, Separators::Windows)),
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
            true,
            || Ok::<_, io::Error>(home.to_owned()),
            String::push_str,
        )
        .unwrap();
        assert_eq!(expanded.as_deref(), expected, "{written:?} over {home:?}");
    }
    let error = super::expand(r"~\x", true, unread, String::push_str).unwrap_err();
    assert_eq!(error.to_string(), "the home directory was read");
}
