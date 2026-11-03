//! Vendors locked dependencies for private fixture Cargo homes.
//!
//! Most fixtures use the small registry closure because Cargo reads every vendored package.
//! The cold browser build resolves the whole workspace instead. Both sets retain stable source
//! paths while their dependency inputs stay unchanged. The complete set also carries the compiled
//! registry units of the cold browser bootstrap, so that build compiles only Maestro's crates.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use sha2::{Digest, Sha256};

use super::{cargo, export};
use crate::copy_assets::copy_tree;
use crate::test_setup::hex;

/// Compile and prune the registry units of the cold browser bootstrap.
mod seed;

/// The directory of the tooling fixtures, from the checkout root.
const FIXTURES: &str = "crates/maestro-tooling/tests/fixtures";

/// The dependency closure a fixture resolves.
#[derive(Clone, Copy, Debug)]
pub(super) enum Scope {
    /// Only registry dependencies declared by the tooling fixtures.
    Fixtures,
    /// Every dependency selected by the workspace and browser locks.
    Workspace,
}

/// Reuse or vendor the selected closure, then export and print its source configuration.
///
/// # Errors
/// Returns a failed copy or write, or the log of a failed `cargo vendor`.
pub(super) fn run(checkout: &Path, scope: Scope) -> io::Result<ExitCode> {
    let (name, variable) = match scope {
        Scope::Fixtures => ("fixture-sources", "MAESTRO_TEST_CARGO_SOURCES"),
        Scope::Workspace => ("workspace-sources", "MAESTRO_TEST_WORKSPACE_CARGO_SOURCES"),
    };
    let key = key(checkout, scope)?;
    let directory = checkout.join("target").join(name).join(&key[..16]);
    let config = directory.join("config.toml");
    if !config.is_file() {
        if directory.exists() {
            std::fs::remove_dir_all(&directory)?;
        }
        std::fs::create_dir_all(&directory)?;
        vendor(checkout, &directory, scope)?;
    }
    if matches!(scope, Scope::Workspace) {
        seed::ensure(checkout, &directory, &config)?;
    }
    let mut entry = OsString::from(format!("{variable}="));
    entry.push(&config);
    export(&entry)?;
    println!("{}", config.display());
    Ok(ExitCode::SUCCESS)
}

/// Hash names and contents of the files that determine the selected dependency set.
fn key(checkout: &Path, scope: Scope) -> io::Result<String> {
    let fixtures = checkout.join(FIXTURES);
    let mut inputs = vec![
        checkout.join("Cargo.lock"),
        fixtures.join("browser_author/Cargo.lock"),
        fixtures.join("browser_author/Cargo.toml"),
    ];
    match scope {
        Scope::Fixtures => inputs.push(fixtures.join("registry_sources/Cargo.toml")),
        Scope::Workspace => {
            // The toolchain decides every compiled unit's hash, so it keys the seed built here.
            inputs.push(checkout.join("rust-toolchain.toml"));
            inputs.push(checkout.join("Cargo.toml"));
            inputs.extend(member_manifests(checkout)?);
        }
    }
    inputs.sort();
    let mut hash = Sha256::new();
    for input in inputs {
        hash.update(input.as_os_str().as_encoded_bytes());
        hash.update([0]);
        hash.update(Sha256::digest(std::fs::read(input)?));
    }
    Ok(hex(&hash.finalize()))
}

/// The manifests of the workspace members. Membership is `crates/*`; fixture manifests below
/// members are separate.
fn member_manifests(checkout: &Path) -> io::Result<Vec<PathBuf>> {
    let mut manifests = Vec::new();
    for entry in std::fs::read_dir(checkout.join("crates"))? {
        let manifest = entry?.path().join("Cargo.toml");
        if manifest.is_file() {
            manifests.push(manifest);
        }
    }
    Ok(manifests)
}

/// Stage one source set and publish its configuration only after Cargo succeeds.
fn vendor(checkout: &Path, directory: &Path, scope: Scope) -> io::Result<()> {
    let fixtures = checkout.join(FIXTURES);
    let mut command = Command::new(cargo());
    command.current_dir(checkout).args(["vendor", "--offline"]);
    let manifest = match scope {
        Scope::Fixtures => {
            let closure = directory.join("closure");
            copy_tree(&fixtures.join("registry_sources"), &closure)?;
            std::fs::copy(checkout.join("Cargo.lock"), closure.join("Cargo.lock"))?;
            closure.join("Cargo.toml")
        }
        Scope::Workspace => {
            command.arg("--locked");
            checkout.join("Cargo.toml")
        }
    };
    let vendored = command
        .arg("--manifest-path")
        .arg(manifest)
        .arg("--sync")
        .arg(fixtures.join("browser_author/Cargo.toml"))
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
    std::fs::write(directory.join("config.toml"), source)
}
