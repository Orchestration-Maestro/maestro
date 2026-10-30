//! Vendors the registry crates the tooling fixtures resolve, so their private Cargo homes never
//! touch the shared package cache.
//!
//! Cargo reads every vendored package on each run, so the set holds only the fixture closure:
//! the crates named by the registry-sources manifest, at the versions the workspace lock
//! selects, and the browser fixture's crates. A directory keyed by those inputs is reused across
//! runs; one preparation per checkout runs at a time.

use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::{Command, ExitCode};

use sha2::{Digest, Sha256};

use super::{cargo, export};
use crate::copy_assets::copy_tree;
use crate::test_setup::hex;

/// The directory of the tooling fixtures, from the checkout root.
const FIXTURES: &str = "crates/maestro-tooling/tests/fixtures";

/// Vendor the closure unless this input set already has a directory, then export and print the
/// path of its source configuration.
///
/// # Errors
/// Returns a failed copy or write, or the log of a failed `cargo vendor`.
pub(super) fn run(checkout: &Path) -> io::Result<ExitCode> {
    let fixtures = checkout.join(FIXTURES);
    let browser = fixtures.join("browser_author");
    let mut hash = Sha256::new();
    for input in [
        checkout.join("Cargo.lock"),
        browser.join("Cargo.lock"),
        fixtures.join("registry_sources/Cargo.toml"),
    ] {
        hash.update(Sha256::digest(std::fs::read(input)?));
    }
    let key = hex(&hash.finalize());
    let directory = checkout.join("target/fixture-sources").join(&key[..16]);
    let config = directory.join("config.toml");
    if !config.is_file() {
        if directory.exists() {
            std::fs::remove_dir_all(&directory)?;
        }
        std::fs::create_dir_all(&directory)?;
        let closure = directory.join("closure");
        copy_tree(&fixtures.join("registry_sources"), &closure)?;
        std::fs::copy(checkout.join("Cargo.lock"), closure.join("Cargo.lock"))?;
        let vendored = Command::new(cargo())
            .current_dir(checkout)
            .args(["vendor", "--offline", "--manifest-path"])
            .arg(closure.join("Cargo.toml"))
            .arg("--sync")
            .arg(browser.join("Cargo.toml"))
            .arg(directory.join("vendor"))
            .output()?;
        if !vendored.status.success() {
            eprint!("{}", String::from_utf8_lossy(&vendored.stderr));
            return Err(io::Error::other(format!(
                "cargo vendor failed: {}",
                vendored.status
            )));
        }
        let mut source = vendored.stdout;
        source.extend_from_slice(b"\n[net]\noffline = true\n");
        std::fs::write(&config, source)?;
    }
    let mut entry = OsString::from("MAESTRO_TEST_CARGO_SOURCES=");
    entry.push(&config);
    export(&entry)?;
    println!("{}", config.display());
    Ok(ExitCode::SUCCESS)
}
