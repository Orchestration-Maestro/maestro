//! Home-directory expansion of authored paths over supplied homes.
#![cfg(test)]

use maestro_path::{expand_tilde_native, expand_tilde_with_flavor};
use std::{io, path::PathBuf};

/// Expands `path` in one flavor over a fixed home.
fn expand(path: &str, windows: bool, home: &str) -> String {
    expand_tilde_with_flavor::<io::Error>(path, windows, || Ok(home.to_owned())).unwrap()
}

/// A home lookup that must not run.
fn unread() -> io::Result<String> {
    Err(io::Error::other("the home directory was read"))
}

/// An exact `~` is the home, `~/rest` joins the rest as written, and every other spelling stays.
#[test]
fn tilde_rule_is_one() {
    let posix = [
        ("~", "/h", "/h"),
        ("~", "/h/.", "/h/."),
        ("~", "", ""),
        ("~/", "/h", "/h/"),
        ("~/", "", ""),
        ("~/x", "", "x"),
        ("~/x", "/h", "/h/x"),
        ("~//x", "/h", "/h//x"),
        ("~/../x", "/h", "/h/../x"),
        ("~x", "/h", "~x"),
        ("~user/x", "/h", "~user/x"),
        (r"~\x", "/h", r"~\x"),
        ("./~", "/h", "./~"),
        ("a~/x", "/h", "a~/x"),
        ("", "/h", ""),
    ];
    for (written, home, expected) in posix {
        assert_eq!(expand(written, false, home), expected, "posix {written:?}");
    }
}

/// On Windows `~\x` expands, either separator joins natively and empty components drop.
#[test]
fn windows_tilde_joins_natively() {
    let rows = [
        ("~", r"C:\h", r"C:\h"),
        (r"~\", r"C:\h", r"C:\h"),
        ("~/", r"C:\h", r"C:\h"),
        (r"~\x", r"C:\h", r"C:\h\x"),
        ("~/x", r"C:\h", r"C:\h\x"),
        (r"~/a\b/c", r"C:\h", r"C:\h\a\b\c"),
        (r"~\\x//y", r"C:\h", r"C:\h\x\y"),
        ("~/x", r"C:\h\\", r"C:\h\\x"),
        (r"~\..\x", r"C:\h", r"C:\h\..\x"),
        ("~/x", "", "x"),
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

/// A native home keeps its bytes and a native join follows the same rule as the text one.
#[cfg(unix)]
#[test]
fn native_tilde_keeps_home_bytes() {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
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
