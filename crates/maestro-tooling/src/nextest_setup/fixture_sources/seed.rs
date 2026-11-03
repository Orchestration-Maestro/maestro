//! The compiled registry units the cold browser bootstrap starts from.
//!
//! The cold bootstrap proves that Maestro's own crates build from empty state. Compiling every
//! registry crate beside them repeats work no Maestro change affects, so the setup builds the
//! bootstrap's native targets once per dependency key and keeps only what the registry crates
//! produced: the test copies that into its empty target directory.

use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::process::Stdio;

use super::member_manifests;
use crate::browser_author::bootstrap_commands;
use crate::nextest_setup::{browser_graph::isolate, cargo};
use crate::test_setup::remove_directory;

/// The published seed, beside the source configuration it was built against.
const SEED: &str = "seed";

/// The target directory the seed is built in, kept across a failed attempt.
const BUILDING: &str = "seed-target";

/// The directories of a profile directory that hold compiled units.
const UNITS: [&str; 3] = [".fingerprint", "build", "deps"];

/// Build the seed in `directory` against the source configuration `sources`, unless it exists.
///
/// # Errors
/// Returns the failed build command or a failed prune.
pub(super) fn ensure(checkout: &Path, directory: &Path, sources: &Path) -> io::Result<()> {
    let seed = directory.join(SEED);
    if seed.is_dir() {
        return Ok(());
    }
    let building = directory.join(BUILDING);
    for mut build in bootstrap_commands(Path::new(&cargo()), checkout) {
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
    for entry in std::fs::read_dir(profile)? {
        let entry = entry?;
        let path = entry.path();
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| UNITS.contains(&name))
        {
            remove_member_units(&path, members)?;
        } else if entry.file_type()?.is_dir() {
            remove_directory(&path)?;
        } else {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

/// Delete the entries of `units` that belong to a workspace member.
fn remove_member_units(units: &Path, members: &HashSet<String>) -> io::Result<()> {
    for entry in std::fs::read_dir(units)? {
        let entry = entry?;
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| is_member_unit(name, members))
        {
            if entry.file_type()?.is_dir() {
                remove_directory(&entry.path())?;
            } else {
                std::fs::remove_file(entry.path())?;
            }
        }
    }
    Ok(())
}

/// Whether the unit entry `name`, `<crate>-<hash>` with any extension and `lib` prefix, belongs
/// to a member. Fingerprints and build directories spell the package name, outputs the crate name.
fn is_member_unit(name: &str, members: &HashSet<String>) -> bool {
    let stem = name.split('.').next().unwrap_or(name);
    let Some((unit, hash)) = stem.rsplit_once('-') else {
        return false;
    };
    if hash.len() != 16 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return false;
    }
    let unit = unit.replace('-', "_");
    members.contains(&unit)
        || unit
            .strip_prefix("lib")
            .is_some_and(|name| members.contains(name))
}
