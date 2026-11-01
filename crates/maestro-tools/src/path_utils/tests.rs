//! Home-form recognition before the home lookup.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use super::expand_written_path_with;
use std::io;

/// Only the two expanded forms read the home directory.
#[test]
fn only_the_expanded_forms_read_the_home_directory() {
    let failing = || Err(io::Error::new(io::ErrorKind::NotFound, "unknown home"));
    assert_eq!(
        expand_written_path_with("~other/file", failing).unwrap(),
        "~other/file"
    );
    assert_eq!(
        expand_written_path_with("~/x", failing).unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
}

/// Only forward-slash home forms expand, keeping the suffix exactly as authored.
#[test]
fn directory_paths_expand_only_forward_slash_home_forms() {
    for (written, expected) in [
        ("~/a//b", r"C:\h/a//b"),
        ("~/x", r"C:\h/x"),
        ("~/", r"C:\h/"),
        ("~", r"C:\h"),
    ] {
        assert_eq!(
            expand_written_path_with(written, || Ok(r"C:\h".to_owned())).unwrap(),
            expected
        );
    }
    assert_eq!(
        expand_written_path_with("~/x", || Ok("/h/".to_owned())).unwrap(),
        "/h//x"
    );
    for written in [r"~\x", r"~\"] {
        let unread = || Err(io::Error::other("home was read"));
        assert_eq!(expand_written_path_with(written, unread).unwrap(), written);
    }
}
