//! The compiled registry units the cold browser bootstrap starts from.
//!
//! The cold bootstrap proves that Maestro's own crates build from empty state. Compiling every
//! registry crate beside them repeats work no Maestro change affects, so the setup builds the
//! bootstrap's native targets once per dependency key and keeps only what the registry crates
//! produced: the test copies that into its empty target directory. A compiled unit records the
//! path of the source it was built from, so a seed serves only the sources it was compiled
//! against: a shard takes the bundle's seed when the bundle compiled it beside the same
//! directory, and builds its own otherwise.

use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::process::Stdio;

use super::member_manifests;
use crate::browser_author::bootstrap_commands;
use crate::copy_assets::copy_tree;
use crate::nextest_setup::{browser_graph::isolate, prepared_bundle};
use crate::test_setup::{install_seed, remove_directory};

/// The published seed, beside the source configuration it was built against.
const SEED: &str = "seed";

/// The target directory the seed is built or installed in, kept across a failed build.
const BUILDING: &str = "seed-target";

/// The directories of a profile directory that hold compiled units.
const UNITS: [&str; 3] = [".fingerprint", "build", "deps"];

/// Install the bundle's seed in `directory`, or build it there with `cargo` against the source
/// configuration `sources`, unless it exists.
///
/// # Errors
/// Returns an unreadable bundle, a failed copy, the failed build command or a failed prune.
pub(super) fn ensure(
    cargo: &Path,
    checkout: &Path,
    directory: &Path,
    sources: &Path,
) -> io::Result<()> {
    let seed = directory.join(SEED);
    if seed.is_dir() {
        return Ok(());
    }
    let building = directory.join(BUILDING);
    if let Some(bundle) = prepared_bundle(checkout)
        && install_seed(&bundle, directory, &building)?
    {
        return std::fs::rename(building, seed);
    }
    for mut build in bootstrap_commands(cargo, checkout) {
        isolate(&mut build, &building);
        let status = build
            .arg("--config")
            .arg(sources)
            .stdout(Stdio::from(io::stderr()))
            .status()?;
        if !status.success() {
            return Err(io::Error::other(format!(
                "the cold bootstrap seed did not build: {build:?} {status}"
            )));
        }
    }
    prune(&building.join("debug"), &members(checkout)?)?;
    std::fs::rename(building, seed)
}

/// Copy the seed of `directory` into the bundle directory `staged`.
///
/// # Errors
/// Returns a failed removal or copy.
pub(super) fn stage(directory: &Path, staged: &Path) -> io::Result<()> {
    remove_directory(staged)?;
    copy_tree(&directory.join(SEED), staged)
}

/// The crate names of the workspace members, as their compiled units spell them.
fn members(checkout: &Path) -> io::Result<HashSet<String>> {
    Ok(member_manifests(checkout)?
        .iter()
        .filter_map(|manifest| manifest.parent()?.file_name()?.to_str())
        .map(|name| name.replace('-', "_"))
        .collect())
}

/// Keep the units of registry crates in `profile` and delete everything else: the outputs of the
/// workspace members, their units and the incremental state.
fn prune(profile: &Path, members: &HashSet<String>) -> io::Result<()> {
    let mut removed = HashSet::new();
    // Fingerprints and build directories come before outputs, which are matched by hash.
    for name in UNITS {
        let path = profile.join(name);
        if path.is_dir() {
            remove_member_units(&path, members, &mut removed)?;
        }
    }
    for entry in std::fs::read_dir(profile)? {
        let entry = entry?;
        let path = entry.path();
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| UNITS.contains(&name))
        {
            continue;
        }
        if entry.file_type()?.is_dir() {
            remove_directory(&path)?;
        } else {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

/// Delete the entries of `units` that belong to a workspace member, by name or by the hash of a
/// member unit already removed, and record the hashes removed by name.
///
/// A member's binary compiles to `deps/<binary>-<hash>`, which no package name identifies.
fn remove_member_units(
    units: &Path,
    members: &HashSet<String>,
    removed: &mut HashSet<String>,
) -> io::Result<()> {
    for entry in std::fs::read_dir(units)? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some((unit, hash)) = split_unit(&name) else {
            continue;
        };
        if !is_member(&unit, members) && !removed.contains(&hash) {
            continue;
        }
        removed.insert(hash);
        if entry.file_type()?.is_dir() {
            remove_directory(&entry.path())?;
        } else {
            std::fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

/// The package or crate name and the hash of the unit entry `name`, `<unit>-<hash>` with any
/// extension.
fn split_unit(name: &str) -> Option<(String, String)> {
    let stem = name.split('.').next().unwrap_or(name);
    let (unit, hash) = stem.rsplit_once('-')?;
    (hash.len() == 16 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| (unit.replace('-', "_"), hash.to_owned()))
}

/// Whether `unit` names a member, as its package name or as its library output, which carries the
/// `lib` prefix.
fn is_member(unit: &str, members: &HashSet<String>) -> bool {
    members.contains(unit)
        || unit
            .strip_prefix("lib")
            .is_some_and(|name| members.contains(name))
}
