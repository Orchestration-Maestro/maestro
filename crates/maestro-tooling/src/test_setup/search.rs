//! Pinned search programs carried by the prepared test bundle.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};

use super::{copy, digest, remove_directory, verify};
use crate::search_tools::{PROGRAMS, resolve};

/// The bundle directory that keeps search programs independent of the test PATH.
const DIRECTORY: &str = "search-tools";

/// Resolve each configured pin once and stage it through the bundle's file copier.
pub(super) fn stage(checkout: &Path, output: &Path) -> io::Result<BTreeMap<String, String>> {
    let resolved = resolve(checkout).map_err(io::Error::other)?;
    let directory = output.join(DIRECTORY);
    remove_directory(&directory)?;
    std::fs::create_dir_all(&directory)?;
    let mut programs = BTreeMap::new();
    for (variable, source) in resolved {
        let Some((_, name, _)) = PROGRAMS.iter().find(|(key, _, _)| *key == variable) else {
            return Err(io::Error::other("unknown search program"));
        };
        let destination = directory.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        copy(Path::new(&source), &destination)?;
        programs.insert((*name).to_owned(), digest(&std::fs::read(destination)?));
    }
    Ok(programs)
}

/// Paths supplied to the preparation example after the configured pins have been staged.
pub(super) fn paths(output: &Path) -> Vec<(&'static str, PathBuf)> {
    PROGRAMS
        .into_iter()
        .filter_map(|(variable, name, _)| {
            let path = output
                .join(DIRECTORY)
                .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
            path.is_file().then_some((variable, path))
        })
        .collect()
}

/// Validate every recorded program before exporting its bundle path.
pub(super) fn export(output: &Path, programs: &BTreeMap<String, String>) -> io::Result<String> {
    let mut entries = String::new();
    for (variable, name, _) in PROGRAMS {
        if let Some(expected) = programs.get(name) {
            let path = output
                .join(DIRECTORY)
                .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
            verify(&path, expected)?;
            writeln!(entries, "{variable}={}", path.display()).map_err(io::Error::other)?;
        }
    }
    Ok(entries)
}
