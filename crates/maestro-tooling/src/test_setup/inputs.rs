//! What one prepared artifact is built from, and the digest that tells when it is stale.
//!
//! An artifact names the repository paths it reads and the locked packages it uses. Its digest
//! covers the files those paths list, by name and content, and the `Cargo.lock` entries of
//! those packages, so a change elsewhere in the checkout leaves it valid.

use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{at, hex};
use crate::cargo_lock::Lock;

/// The paths and locked packages an artifact is prepared from, with the digest they had.
#[derive(Serialize, Deserialize)]
pub(super) struct Inputs {
    /// Repository-relative Git pathspecs of the files the artifact reads.
    paths: BTreeSet<String>,
    /// Names of the locked packages whose `Cargo.lock` entries the artifact uses.
    locked: BTreeSet<String>,
    /// Digest of the listed files and of the lock entries.
    pub(super) digest: String,
}

impl Inputs {
    /// Digest the files `paths` list in `checkout` and the lock entries of `locked`.
    ///
    /// # Errors
    /// Returns a failed listing, read or lock parse.
    pub(super) fn new(
        checkout: &Path,
        paths: BTreeSet<String>,
        locked: BTreeSet<String>,
    ) -> io::Result<Self> {
        let digest = digest(checkout, &paths, &locked)?;
        Ok(Self {
            paths,
            locked,
            digest,
        })
    }

    /// Whether `checkout` still holds what the digest was taken from.
    ///
    /// # Errors
    /// Returns a failed listing, read or lock parse.
    pub(super) fn is_current(&self, checkout: &Path) -> io::Result<bool> {
        Ok(digest(checkout, &self.paths, &self.locked)? == self.digest)
    }

    /// Check that `checkout` still holds what the digest was taken from, naming `bundle` when
    /// it does not.
    ///
    /// # Errors
    /// Returns a stale digest, or a failed listing, read or lock parse.
    pub(super) fn verify(&self, checkout: &Path, bundle: &Path) -> io::Result<()> {
        match self.is_current(checkout) {
            Ok(true) => Ok(()),
            Ok(false) => Err(at(bundle, "stale prepared component inputs")),
            Err(error) => Err(at(bundle, &error)),
        }
    }
}

/// Digest the listed files by name and content, then the lock entries.
fn digest(
    checkout: &Path,
    paths: &BTreeSet<String>,
    locked: &BTreeSet<String>,
) -> io::Result<String> {
    let mut hash = Sha256::new();
    for name in listed(checkout, paths.iter().map(String::as_str))? {
        hash.update(name.as_bytes());
        hash.update([0]);
        let path = checkout.join(&name);
        if path.is_file() {
            hash.update(Sha256::digest(std::fs::read(path)?));
        } else {
            hash.update(b"missing");
        }
    }
    for entry in lock_entries(checkout, locked)? {
        hash.update(entry.as_bytes());
    }
    Ok(hex(&hash.finalize()))
}

/// The repository-relative names of the files the pathspecs match, sorted. No pathspec matches
/// no file, where Git would list them all.
pub(super) fn listed<'a>(
    checkout: &Path,
    paths: impl Iterator<Item = &'a str>,
) -> io::Result<BTreeSet<String>> {
    let paths: Vec<&str> = paths.collect();
    if paths.is_empty() {
        return Ok(BTreeSet::new());
    }
    let listing = Command::new("git")
        .current_dir(checkout)
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
        ])
        .args(paths)
        .output()?;
    if !listing.status.success() {
        return Err(io::Error::other(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&listing.stderr)
        )));
    }
    listing
        .stdout
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
        .map(|name| String::from_utf8(name.to_vec()).map_err(io::Error::other))
        .collect()
}

/// The text of every lock entry whose package is named in `locked`, in lock order.
fn lock_entries(checkout: &Path, locked: &BTreeSet<String>) -> io::Result<Vec<String>> {
    if locked.is_empty() {
        return Ok(Vec::new());
    }
    Lock::read(checkout)?
        .packages()
        .iter()
        .filter(|package| locked.contains(&package.name))
        .map(|package| serde_json::to_string(package).map_err(io::Error::other))
        .collect()
}

/// The packages named in `roots` and every locked package they build on, by name.
///
/// # Errors
/// Returns a failed read or parse of `Cargo.lock`.
pub(super) fn locked_closure(
    checkout: &Path,
    roots: &BTreeSet<String>,
) -> io::Result<BTreeSet<String>> {
    Ok(Lock::read(checkout)?
        .graph(|_| true)
        .closure(roots.iter().map(String::as_str))
        .into_iter()
        .map(|package| package.name.clone())
        .collect())
}
