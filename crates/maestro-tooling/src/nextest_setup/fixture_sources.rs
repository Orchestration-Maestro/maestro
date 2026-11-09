//! Vendors locked dependencies for private fixture Cargo homes.
//!
//! Most fixtures use the small registry closure because Cargo reads every vendored package.
//! The cold browser build resolves the whole workspace instead. Both sets retain stable source
//! paths while their dependency inputs stay unchanged. The complete set also carries the compiled
//! registry units of the cold browser bootstrap, so that build compiles only Maestro's crates.
//! The test preparation builds those units once and stages them in the bundle; a shard installs
//! them beside the sources it vendors, or builds them when the bundle compiled them elsewhere.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use sha2::{Digest, Sha256};

use super::export;
use crate::copy_assets::copy_tree;
use crate::test_setup::hex;
use crate::toolchain;

/// Compile and prune the registry units of the cold browser bootstrap.
mod seed;

/// The bundle directory that holds the staged seed.
pub(crate) const STAGED: &str = "workspace-sources";

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

impl Scope {
    /// The directory below `target` that holds the closure, and the variable that exports its
    /// source configuration.
    fn names(self) -> (&'static str, &'static str) {
        match self {
            Self::Fixtures => ("fixture-sources", "MAESTRO_TEST_CARGO_SOURCES"),
            Self::Workspace => ("workspace-sources", "MAESTRO_TEST_WORKSPACE_CARGO_SOURCES"),
        }
    }
}

/// Reuse or vendor the selected closure with the Cargo executable `cargo`, then export and print
/// its source configuration.
///
/// # Errors
/// Returns a failed copy, write or seed build, or the log of a failed `cargo vendor`.
pub(super) fn run(cargo: &Path, checkout: &Path, scope: Scope) -> io::Result<ExitCode> {
    let config = prepare(cargo, checkout, scope)?;
    let mut entry = OsString::from(format!("{}=", scope.names().1));
    entry.push(&config);
    export(&entry)?;
    println!("{}", config.display());
    Ok(ExitCode::SUCCESS)
}

/// Prepare the complete workspace sources and the seed in `checkout`, then stage the seed in the
/// bundle `output`, and return the directory of those sources, which the seed was compiled
/// against.
///
/// # Errors
/// Returns a failed copy, write or seed build, or the log of a failed `cargo vendor`.
pub(crate) fn stage(cargo: &Path, checkout: &Path, output: &Path) -> io::Result<PathBuf> {
    let config = prepare(cargo, checkout, Scope::Workspace)?;
    let directory = config
        .parent()
        .ok_or_else(|| io::Error::other("the source configuration has no directory"))?;
    seed::stage(directory, &output.join(STAGED))?;
    Ok(directory.to_path_buf())
}

/// Reuse or vendor the selected closure, with the seed of the workspace closure, and return the
/// path of its source configuration.
fn prepare(cargo: &Path, checkout: &Path, scope: Scope) -> io::Result<PathBuf> {
    let key = key(checkout, scope)?;
    let directory = checkout
        .join("target")
        .join(scope.names().0)
        .join(&key[..16]);
    let config = directory.join("config.toml");
    if !config.is_file() {
        if directory.exists() {
            std::fs::remove_dir_all(&directory)?;
        }
        std::fs::create_dir_all(&directory)?;
        vendor(cargo, checkout, &directory, scope)?;
    }
    if matches!(scope, Scope::Workspace) {
        seed::ensure(cargo, checkout, &directory, &config)?;
    }
    Ok(config)
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
fn vendor(cargo: &Path, checkout: &Path, directory: &Path, scope: Scope) -> io::Result<()> {
    let fixtures = checkout.join(FIXTURES);
    let mut command = toolchain::command(cargo, checkout);
    command.args(["vendor", "--offline"]);
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
