//! Resolution against the live process directories, which these tests read and never change.
#![cfg(not(target_arch = "wasm32"))]

use maestro_path::fs::{current_dir, resolve, resolve_groups, resolve_groups_with};
use std::io;

/// The process working directory as the standard library reports it.
fn working_directory() -> String {
    std::env::current_dir()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

#[test]
fn an_open_path_resolves_below_the_process_working_directory() {
    let below = maestro_path::join(&[&working_directory(), "leaf"]);

    assert_eq!(current_dir().unwrap(), working_directory());
    assert_eq!(resolve(&["leaf"]).unwrap(), below);
    assert_eq!(
        resolve_groups(&[&["leaf"], &["other", "../leaf"]]).unwrap(),
        [below.clone(), below]
    );
}

#[test]
fn a_supplied_working_directory_is_called_only_when_a_group_is_open() {
    let anchored = working_directory();
    let unreadable = || Err(io::Error::from(io::ErrorKind::PermissionDenied));

    let closed = resolve_groups_with(&[&[&anchored]], unreadable);
    let open = resolve_groups_with(&[&[&anchored], &["leaf"]], unreadable);

    assert_eq!(closed.unwrap(), [anchored]);
    assert_eq!(open.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
}
