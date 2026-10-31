//! Home-directory expansion of authored paths over supplied homes, in the host's flavor.
#![cfg(test)]

use maestro_path::{Tilde, expand_tilde, expand_tilde_joined, parse_tilde};
use std::io;

/// A home lookup that must not run.
fn unread() -> io::Result<String> {
    Err(io::Error::other("the home directory was read"))
}

/// The host-flavor parse agrees with the flavor tables for the spellings both share.
#[test]
fn host_parse_reads_the_three_forms() {
    assert_eq!(parse_tilde("~"), Tilde::Home);
    assert_eq!(parse_tilde("~/x"), Tilde::Below("x"));
    assert_eq!(parse_tilde("~x"), Tilde::Other);
}

/// The concatenating expansion appends the rest below the home as written.
#[test]
fn concatenation_appends_the_rest_below_the_home() {
    let (home, below) = if cfg!(windows) {
        (r"C:\h", r"C:\h/x")
    } else {
        ("/h", "/h/x")
    };
    let text = || Ok::<_, io::Error>(home.to_owned());
    assert_eq!(expand_tilde("~", text).unwrap(), home);
    assert_eq!(expand_tilde("~/x", text).unwrap(), below);
    assert_eq!(expand_tilde("~x", text).unwrap(), "~x");
}

/// A home that ends in a separator gains none, so the result has no doubled separator.
#[cfg(not(windows))]
#[test]
fn concatenation_does_not_double_a_home_separator() {
    for (home, expected) in [("/home/u/", "/home/u/x"), ("/", "/x"), ("", "/x")] {
        let text = || Ok::<_, io::Error>(home.to_owned());
        assert_eq!(expand_tilde("~/x", text).unwrap(), expected, "{home:?}");
    }
}

/// The joined expansion normalizes the rest below the home: `.`, `..` and repeated separators
/// resolve, an exact `~` stays the home as supplied, and `~name` is not a home form.
#[test]
fn joined_expansion_normalizes_below_the_home() {
    let (home, rows) = if cfg!(windows) {
        (
            r"C:\home\sample",
            [
                ("~/link/../f", r"C:\home\sample\f"),
                ("~", r"C:\home\sample"),
                ("~/", r"C:\home\sample"),
                ("~//note", r"C:\home\sample\note"),
                ("~child", "~child"),
            ],
        )
    } else {
        (
            "/home/sample",
            [
                ("~/link/../f", "/home/sample/f"),
                ("~", "/home/sample"),
                ("~/", "/home/sample"),
                ("~//note", "/home/sample/note"),
                ("~child", "~child"),
            ],
        )
    };
    let text = || Ok::<_, io::Error>(home.to_owned());
    for (written, expected) in rows {
        assert_eq!(
            expand_tilde_joined(written, text).unwrap(),
            expected,
            "{written:?}"
        );
    }
    let unexpanded = "/extra/../extra/q.md";
    assert_eq!(expand_tilde_joined(unexpanded, unread).unwrap(), unexpanded);
}

/// An exact `~` returns the home unnormalized, while the rest of `~/` is joined.
#[cfg(not(windows))]
#[test]
fn joined_expansion_keeps_an_exact_home_as_supplied() {
    let text = || Ok::<_, io::Error>("/h/.".to_owned());
    assert_eq!(expand_tilde_joined("~", text).unwrap(), "/h/.");
    assert_eq!(expand_tilde_joined("~/x", text).unwrap(), "/h/x");
}

/// An empty home joins lexically, so `~/` names the working directory and `~/x` is relative.
#[test]
fn joined_expansion_joins_an_empty_home_lexically() {
    let text = || Ok::<_, io::Error>(String::new());
    assert_eq!(expand_tilde_joined("~/", text).unwrap(), ".");
    assert_eq!(expand_tilde_joined("~", text).unwrap(), "");
    assert_eq!(expand_tilde_joined("~/x", text).unwrap(), "x");
    assert_eq!(expand_tilde_joined("~child", text).unwrap(), "~child");
}

/// Only the two expanded forms read the home directory, and its error comes back.
#[test]
fn joined_home_is_read_only_for_the_expanded_forms() {
    for written in ["", "x", "~x", "~user/x", "a~", "./~", "/~"] {
        assert_eq!(expand_tilde_joined(written, unread).unwrap(), written);
    }
    for written in ["~", "~/", "~/x"] {
        let error = expand_tilde_joined(written, unread).unwrap_err();
        assert_eq!(error.to_string(), "the home directory was read");
    }
}

/// A native home keeps its bytes and a native expansion follows the same rule as the text one.
#[cfg(unix)]
#[test]
fn native_tilde_keeps_home_bytes() {
    use maestro_path::expand_tilde_native;
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt, path::PathBuf};
    let home = PathBuf::from(OsStr::from_bytes(b"/home-\xff"));
    let expand = |path: &str| expand_tilde_native::<io::Error>(path, || Ok(home.clone())).unwrap();
    assert_eq!(expand("~"), home);
    assert_eq!(
        expand("~/child"),
        PathBuf::from(OsStr::from_bytes(b"/home-\xff/child"))
    );
    assert_eq!(
        expand("~//x"),
        PathBuf::from(OsStr::from_bytes(b"/home-\xff//x"))
    );
    assert_eq!(expand("~x"), PathBuf::from("~x"));
    let read = expand_tilde_native("x/~", || Err::<PathBuf, _>(io::Error::other("read")));
    assert_eq!(read.unwrap(), PathBuf::from("x/~"));
}
