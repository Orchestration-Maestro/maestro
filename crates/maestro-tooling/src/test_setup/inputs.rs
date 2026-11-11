//! What one prepared artifact is built from, and the digest that tells when it is stale.
//!
//! An artifact names the repository paths it reads and the locked packages it uses. Its digest
//! covers the files those paths list, by name and content, and the `Cargo.lock` entries of
//! those packages, so a change elsewhere in the checkout leaves it valid. A listed manifest can
//! instead be digested without its development dependencies, for a build that never compiles
//! them, and the workspace manifest as a release build of its members reads it.

use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{WORKSPACE_MANIFEST, at, hex};
use crate::cargo_lock::Lock;

/// The paths and locked packages an artifact is prepared from, with the digest they had.
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Inputs {
    /// Repository-relative Git pathspecs of the files the artifact reads.
    paths: BTreeSet<String>,
    /// Names of the locked packages whose `Cargo.lock` entries the artifact uses.
    locked: BTreeSet<String>,
    /// Repository-relative manifests digested without their development dependencies.
    manifests: BTreeSet<String>,
    /// The `[workspace.dependencies]` entries the members inherit, when the workspace manifest is
    /// digested as a release build of them reads it.
    workspace_dependencies: Option<BTreeSet<String>>,
    /// Digest of the listed files, the manifests, the workspace manifest when it is read in part
    /// and the lock entries.
    pub(super) digest: String,
}

impl Inputs {
    /// Digest the files `paths` list in `checkout`, the `manifests` without their development
    /// dependencies and the lock entries of `locked`.
    ///
    /// # Errors
    /// Returns a failed listing, read or lock parse.
    pub(super) fn new(
        checkout: &Path,
        paths: BTreeSet<String>,
        locked: BTreeSet<String>,
        manifests: BTreeSet<String>,
    ) -> io::Result<Self> {
        let digest = digest(checkout, &paths, &locked, &manifests, None)?;
        Ok(Self {
            paths,
            locked,
            manifests,
            workspace_dependencies: None,
            digest,
        })
    }

    /// Digest what `new` digests and the workspace manifest as a release build of its members
    /// reads it, keeping the `workspace_dependencies` entries of its `[workspace.dependencies]`.
    ///
    /// # Errors
    /// Returns a failed listing, read, manifest parse or lock parse.
    pub(super) fn release(
        checkout: &Path,
        paths: BTreeSet<String>,
        locked: BTreeSet<String>,
        manifests: BTreeSet<String>,
        workspace_dependencies: BTreeSet<String>,
    ) -> io::Result<Self> {
        let digest = digest(
            checkout,
            &paths,
            &locked,
            &manifests,
            Some(&workspace_dependencies),
        )?;
        Ok(Self {
            paths,
            locked,
            manifests,
            workspace_dependencies: Some(workspace_dependencies),
            digest,
        })
    }

    /// The repository-relative names of the files the paths list in `checkout`.
    ///
    /// # Errors
    /// Returns a failed listing.
    pub(super) fn files(&self, checkout: &Path) -> io::Result<BTreeSet<String>> {
        listed(checkout, self.paths.iter().map(String::as_str))
    }

    /// Whether `checkout` still holds what the digest was taken from.
    ///
    /// # Errors
    /// Returns a failed listing, read or lock parse.
    pub(super) fn is_current(&self, checkout: &Path) -> io::Result<bool> {
        let digest = digest(
            checkout,
            &self.paths,
            &self.locked,
            &self.manifests,
            self.workspace_dependencies.as_ref(),
        )?;
        Ok(digest == self.digest)
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

/// Digest the listed files by name and content, then the manifests without their development
/// dependencies, then the workspace manifest as a release build reads it with the
/// `workspace_dependencies` entries, when they are given, then the lock entries.
fn digest(
    checkout: &Path,
    paths: &BTreeSet<String>,
    locked: &BTreeSet<String>,
    manifests: &BTreeSet<String>,
    workspace_dependencies: Option<&BTreeSet<String>>,
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
    for name in manifests {
        hash.update(name.as_bytes());
        hash.update([0]);
        hash.update(without_development(&checkout.join(name))?.as_bytes());
    }
    if let Some(inherited) = workspace_dependencies {
        hash.update(WORKSPACE_MANIFEST.as_bytes());
        hash.update([0]);
        let manifest = release_manifest(&checkout.join(WORKSPACE_MANIFEST), inherited)?;
        hash.update(manifest.as_bytes());
    }
    for entry in lock_entries(checkout, locked)? {
        hash.update(entry.as_bytes());
    }
    Ok(hex(&hash.finalize()))
}

/// The manifest at `path` as a table, or none when it does not exist.
fn table(path: &Path) -> io::Result<Option<toml::Table>> {
    match std::fs::read_to_string(path) {
        Ok(text) => text.parse().map(Some).map_err(|error| at(path, &error)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Remove the development dependency tables of a manifest or of one of its target tables.
fn remove_development(table: &mut toml::Table) {
    table.remove("dev-dependencies");
    table.remove("dev_dependencies");
}

/// The manifest at `path` without its development dependencies, which a build of the package
/// as a dependency never compiles, and without the target tables that leaves empty. A missing
/// manifest reads as empty.
fn without_development(path: &Path) -> io::Result<String> {
    let Some(mut manifest) = table(path)? else {
        return Ok(String::new());
    };
    remove_development(&mut manifest);
    if let Some(targets) = manifest
        .get_mut("target")
        .and_then(toml::Value::as_table_mut)
    {
        targets
            .iter_mut()
            .filter_map(|(_, target)| target.as_table_mut())
            .for_each(remove_development);
        targets.retain(|_, target| target.as_table().is_none_or(|table| !table.is_empty()));
        if targets.is_empty() {
            manifest.remove("target");
        }
    }
    Ok(manifest.to_string())
}

/// The workspace manifest at `path` as a release build of its members reads it: without the
/// member lists, the tool metadata, the profiles other than `release` and the entries of
/// `[workspace.dependencies]` outside `inherited`. A missing manifest reads as empty.
fn release_manifest(path: &Path, inherited: &BTreeSet<String>) -> io::Result<String> {
    let Some(mut manifest) = table(path)? else {
        return Ok(String::new());
    };
    if let Some(workspace) = manifest
        .get_mut("workspace")
        .and_then(toml::Value::as_table_mut)
    {
        for unread in ["members", "exclude", "default-members", "metadata"] {
            workspace.remove(unread);
        }
        retain_entries(workspace, "dependencies", |name| inherited.contains(name));
    }
    retain_entries(&mut manifest, "profile", |name| name == "release");
    Ok(manifest.to_string())
}

/// Keep the entries of the table `key` of `table` whose names `keep` accepts, and drop that table
/// when none remains, so a manifest without them reads the same.
fn retain_entries(table: &mut toml::Table, key: &str, keep: impl Fn(&str) -> bool) {
    let Some(entries) = table.get_mut(key).and_then(toml::Value::as_table_mut) else {
        return;
    };
    entries.retain(|name, _| keep(name));
    if entries.is_empty() {
        table.remove(key);
    }
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
