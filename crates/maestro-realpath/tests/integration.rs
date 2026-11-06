//! Real paths of this crate's own files, resolved through the native file system.
#![cfg(not(target_arch = "wasm32"))]

use std::{io, path::Path};

/// This crate's directory, which every test run has on disk.
fn crate_directory() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn two_spellings_of_one_file_resolve_to_one_real_path() {
    let direct = maestro_realpath::canonicalize(crate_directory().join("Cargo.toml")).unwrap();

    let indirect = maestro_realpath::canonicalize(crate_directory().join("src/../Cargo.toml"));

    assert_eq!(indirect.unwrap(), direct);
    assert!(direct.is_absolute() && direct.ends_with("Cargo.toml"));
}

#[test]
fn real_paths_carry_no_verbatim_prefix() {
    let real = maestro_realpath::canonicalize(crate_directory()).unwrap();

    assert!(
        !real.to_string_lossy().starts_with(r"\\?\"),
        "{}",
        real.display()
    );
}

#[test]
fn a_missing_path_reports_not_found() {
    let missing = maestro_realpath::canonicalize(crate_directory().join("no-such-entry"));

    assert_eq!(missing.unwrap_err().kind(), io::ErrorKind::NotFound);
}
