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
