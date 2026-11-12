//! Home-directory text conversion over an observed native home.

use maestro_path::home_directory_text;
use std::io;

#[test]
fn an_observed_home_becomes_its_own_text() {
    let home = std::path::PathBuf::from("/home/user");
    assert_eq!(home_directory_text(Some(home)).unwrap(), "/home/user");
}

#[test]
fn an_unknown_home_is_a_not_found_error() {
    let error = home_directory_text(None).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
}

#[cfg(unix)]
#[test]
fn a_home_that_is_not_utf8_is_an_invalid_data_error() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt, path::PathBuf};

    let home = PathBuf::from(OsString::from_vec(vec![b'/', 0x80]));
    let error = home_directory_text(Some(home)).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}
