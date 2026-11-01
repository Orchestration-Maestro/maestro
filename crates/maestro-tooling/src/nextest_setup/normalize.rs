//! Gives the executables of an artifact directory one modification time.
//!
//! The pinned engine's cache namespace includes the modification time of the executable that
//! consumes it, so the setup example and the test binaries must share a time. The installed
//! browser graph shares one time as well, so Cargo finds no output newer than what builds on it.

use std::collections::{HashMap, HashSet};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::SystemTime;

use super::nonempty;
use crate::test_setup::EXAMPLE;

/// The extensions of the library outputs that can outdate an executable.
const LIBRARY_EXTENSIONS: [&str; 6] = ["rlib", "rmeta", "a", "so", "dylib", "dll"];

/// The exit status that reports an outdated setup example.
const EXAMPLE_OUTDATED: u8 = 3;

/// A library output in the artifact directory.
struct Library {
    /// The file name.
    name: String,
    /// The time the file was last modified.
    modified: SystemTime,
}

/// Give every file under `directory` the modification time `time`, so that none outdates another
/// and, for a time past the checkout's, none outdates a checkout file.
///
/// # Errors
/// Returns a failed read or write.
pub(super) fn set_tree_time(directory: &Path, time: SystemTime) -> io::Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            set_tree_time(&entry.path(), time)?;
        } else if kind.is_file() {
            std::fs::File::options()
                .write(true)
                .open(entry.path())?
                .set_modified(time)?;
        }
    }
    Ok(())
}

/// Whether nextest runs an archive, where nothing is rebuilt and every executable is current.
pub(super) fn archive_signalled() -> bool {
    nonempty("TEST_ARCHIVE").is_some()
}

/// Run the normalization the command line asks for, with the exit status it reports.
///
/// # Errors
/// Returns a directory without executables, malformed Cargo records or a failed read or write.
pub(super) fn run(directory: &Path, archived: bool) -> io::Result<ExitCode> {
    Ok(match normalize(directory, archived)? {
        Outcome::Normalized => ExitCode::SUCCESS,
        Outcome::ExampleOutdated => ExitCode::from(EXAMPLE_OUTDATED),
    })
}

/// What normalization left behind.
#[derive(PartialEq, Eq)]
pub(super) enum Outcome {
    /// Every current executable shares the newest time.
    Normalized,
    /// The setup example is older than a library it builds on, so only a rebuild can serve.
    ExampleOutdated,
}

/// Advance every current executable of the profile `directory` to the newest one's time.
///
/// Cargo rebuilds an executable only while a library output it builds on is newer than it, so
/// an outdated executable keeps its time. Nextest never runs Cargo on an archive, so an
/// archived run has no outdated executable.
///
/// # Errors
/// Returns a directory without executables, malformed Cargo records or a failed read or write.
pub(super) fn normalize(directory: &Path, archived: bool) -> io::Result<Outcome> {
    let fingerprints = if archived {
        Fingerprints::default()
    } else {
        fingerprint_files(directory)?
    };
    let libraries = libraries(&directory.join("deps"))?;
    let newest_library = libraries.iter().map(|library| library.modified).max();
    let mut found = false;
    let mut stale = false;
    let mut current: Vec<(PathBuf, SystemTime)> = Vec::new();
    for artifact in executables(directory)? {
        found = true;
        let modified = std::fs::metadata(&artifact)?.modified()?;
        let outdated = !archived
            && outdated(
                &artifact,
                modified,
                &fingerprints,
                &libraries,
                newest_library,
            )?;
        if outdated {
            stale |= is_example(&artifact);
        } else {
            current.push((artifact, modified));
        }
    }
    if !found {
        return Err(io::Error::other(format!(
            "No consuming executables found in {}.",
            directory.display()
        )));
    }
    if let Some(newest) = current.iter().map(|(_, modified)| *modified).max() {
        for (artifact, modified) in &current {
            if *modified != newest {
                std::fs::File::options()
                    .write(true)
                    .open(artifact)?
                    .set_modified(newest)?;
            }
        }
    }
    if stale {
        eprintln!("The setup example is older than a library it builds on.");
        return Ok(Outcome::ExampleOutdated);
    }
    Ok(Outcome::Normalized)
}

/// The library outputs in `deps`.
fn libraries(deps: &Path) -> io::Result<Vec<Library>> {
    let mut libraries = Vec::new();
    let Ok(entries) = std::fs::read_dir(deps) else {
        return Ok(libraries);
    };
    for entry in entries {
        let path = entry?.path();
        let extension = path.extension().and_then(|extension| extension.to_str());
        if path.is_file()
            && extension.is_some_and(|extension| LIBRARY_EXTENSIONS.contains(&extension))
        {
            libraries.push(Library {
                name: file_name(&path),
                modified: std::fs::metadata(&path)?.modified()?,
            });
        }
    }
    Ok(libraries)
}

/// The final component of `path` as text.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The executables of the profile directory, `deps` and `examples`, each directory in name
/// order. A name with an extension other than `.exe` belongs to something else.
fn executables(directory: &Path) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    for folder in [
        directory.to_path_buf(),
        directory.join("deps"),
        directory.join("examples"),
    ] {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        let mut paths = entries
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<io::Result<Vec<_>>>()?;
        paths.sort();
        found.extend(paths.into_iter().filter(|path| {
            let name = file_name(path);
            (path.extension().is_some_and(|extension| extension == "exe") || !name.contains('.'))
                && is_executable(path)
        }));
    }
    Ok(found)
}

/// Whether `path` is a regular file the system can run.
fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}

/// Whether `artifact` is the setup example, whose staleness the launcher must repair.
fn is_example(artifact: &Path) -> bool {
    artifact
        .parent()
        .is_some_and(|parent| parent.ends_with("examples"))
        && matches!(
            file_name(artifact).as_str(),
            EXAMPLE | "prepare_author_component.exe"
        )
}

/// Whether an exact linked library build is newer than the executable.
fn outdated(
    artifact: &Path,
    modified: SystemTime,
    fingerprints: &Fingerprints,
    libraries: &[Library],
    newest_library: Option<SystemTime>,
) -> io::Result<bool> {
    let Some(record) = executable_fingerprint(artifact, fingerprints)? else {
        return Ok(newest_library.is_some_and(|newest| newest > modified));
    };
    let Some(linked) = linked_libraries(record, fingerprints)? else {
        return Ok(true);
    };
    Ok(libraries.iter().any(|library| {
        let stem = Path::new(&library.name)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default();
        (linked.contains(stem) || linked.contains(stem.strip_prefix("lib").unwrap_or(stem)))
            && library.modified > modified
    }))
}

/// Cargo's dependencies, including their compiled-build fingerprints.
#[derive(Debug, serde::Deserialize, PartialEq, Eq)]
struct Fingerprint {
    /// Package identity, dependency alias, public marker and compiled-build fingerprint.
    deps: Vec<(u64, String, bool, u64)>,
}

/// Cargo records indexed by compiled fingerprint, independent of dependency aliases.
#[derive(Debug, Default)]
struct Fingerprints {
    /// Executable records whose parent directory names their metadata hash.
    executables: Vec<PathBuf>,
    /// Library records keyed by their compiled-build fingerprint.
    libraries: HashMap<u64, Vec<PathBuf>>,
}

/// The fingerprint JSON files Cargo has written under one profile directory.
fn fingerprint_files(directory: &Path) -> io::Result<Fingerprints> {
    let entries = match std::fs::read_dir(directory.join(".fingerprint")) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Fingerprints::default()),
        Err(error) => return Err(error),
    };
    let mut files = Fingerprints::default();
    for entry in entries {
        for file in std::fs::read_dir(entry?.path())? {
            let path = file?.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            if file_name(&path).starts_with("lib-") {
                let hash = fingerprint_hash(&path.with_extension(""))?;
                files.libraries.entry(hash).or_default().push(path);
            } else {
                files.executables.push(path);
            }
        }
    }
    Ok(files)
}

/// Read a Cargo dependency record, refusing malformed records instead of guessing its inputs.
fn read_fingerprint(path: &Path) -> io::Result<Fingerprint> {
    serde_json::from_slice(&std::fs::read(path)?).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid Cargo fingerprint {}: {error}", path.display()),
        )
    })
}

/// The executable record whose directory has the artifact's metadata hash.
fn hashed_fingerprint(
    artifact: &Path,
    fingerprints: &Fingerprints,
) -> io::Result<Option<Fingerprint>> {
    let name = artifact
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let Some((_, hash)) = name.rsplit_once('-') else {
        return Ok(None);
    };
    let mut found = None;
    for path in &fingerprints.executables {
        let directory = path.parent().map(file_name).unwrap_or_default();
        let record = file_name(path);
        if directory
            .rsplit_once('-')
            .is_some_and(|(_, suffix)| suffix == hash)
            && (record.starts_with("test-")
                || record.starts_with("bin-")
                || record.starts_with("example-"))
        {
            let next = read_fingerprint(path)?;
            if found.as_ref().is_some_and(|previous| *previous != next) {
                return Err(io::Error::other(format!(
                    "ambiguous Cargo fingerprints for {}",
                    artifact.display()
                )));
            }
            found = Some(next);
        }
    }
    Ok(found)
}

/// Resolve an uplifted executable through a byte-identical hashed artifact, on every platform.
fn executable_fingerprint(
    artifact: &Path,
    fingerprints: &Fingerprints,
) -> io::Result<Option<Fingerprint>> {
    if let Some(record) = hashed_fingerprint(artifact, fingerprints)? {
        return Ok(Some(record));
    }
    let parent = artifact
        .parent()
        .ok_or_else(|| io::Error::other("executable has no parent"))?;
    let folder = if parent.ends_with("examples") || parent.ends_with("deps") {
        parent.to_path_buf()
    } else {
        parent.join("deps")
    };
    let prefix = format!(
        "{}-",
        artifact
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
    );
    let mut found = None;
    let entries = match std::fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let twin = entry?.path();
        if !file_name(&twin).starts_with(&prefix)
            || !is_executable(&twin)
            || !same_contents(artifact, &twin)?
        {
            continue;
        }
        if let Some(next) = hashed_fingerprint(&twin, fingerprints)? {
            if found.as_ref().is_some_and(|previous| *previous != next) {
                return Err(io::Error::other(format!(
                    "ambiguous hashed twins for {}",
                    artifact.display()
                )));
            }
            found = Some(next);
        }
    }
    Ok(found)
}

/// Compare equal-sized artifacts without retaining their executable images in memory.
fn same_contents(left: &Path, right: &Path) -> io::Result<bool> {
    let mut left = std::fs::File::open(left)?;
    let mut right = std::fs::File::open(right)?;
    let mut remaining = left.metadata()?.len();
    if remaining != right.metadata()?.len() {
        return Ok(false);
    }
    let mut a = [0; 8192];
    let mut b = [0; 8192];
    while remaining > 0 {
        let count = usize::try_from(remaining.min(8192)).map_err(io::Error::other)?;
        left.read_exact(&mut a[..count])?;
        right.read_exact(&mut b[..count])?;
        if a[..count] != b[..count] {
            return Ok(false);
        }
        remaining -= u64::try_from(count).map_err(io::Error::other)?;
    }
    Ok(true)
}

/// Resolve linked outputs, or return `None` when a recorded dependency build no longer exists.
fn linked_libraries(
    record: Fingerprint,
    fingerprints: &Fingerprints,
) -> io::Result<Option<HashSet<String>>> {
    let mut linked = HashSet::new();
    let mut pending = record.deps;
    let mut seen = HashSet::new();
    while let Some((_, alias, _, hash)) = pending.pop() {
        // Build-script results influence compilation but are not linked library outputs.
        if alias == "build_script_build" || !seen.insert(hash) {
            continue;
        }
        let Some(records) = fingerprints.libraries.get(&hash) else {
            return Ok(None);
        };
        for path in records {
            let directory = path.parent().map(file_name).unwrap_or_default();
            let (_, metadata) = directory.rsplit_once('-').ok_or_else(|| {
                io::Error::other("Cargo fingerprint directory has no metadata hash")
            })?;
            let name = path
                .file_stem()
                .and_then(|name| name.to_str())
                .and_then(|name| name.strip_prefix("lib-"))
                .ok_or_else(|| io::Error::other("Cargo library record has no crate name"))?;
            linked.insert(format!("{name}-{metadata}"));
            pending.extend(read_fingerprint(path)?.deps);
        }
    }
    Ok(Some(linked))
}

/// Decode the eight bytes Cargo writes as little-endian hexadecimal text.
fn fingerprint_hash(path: &Path) -> io::Result<u64> {
    let text = std::fs::read_to_string(path)?;
    if text.len() != 16 {
        return Err(io::Error::other(format!(
            "invalid Cargo fingerprint hash {}",
            path.display()
        )));
    }
    u64::from_str_radix(&text, 16)
        .map(u64::swap_bytes)
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid Cargo fingerprint hash {}: {error}", path.display()),
            )
        })
}
