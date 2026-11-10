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
use crate::test_setup::{EXAMPLE, at};

/// The extensions of the library outputs that can outdate an executable.
const LIBRARY_EXTENSIONS: [&str; 6] = ["rlib", "rmeta", "a", "so", "dylib", "dll"];

/// The exit status that reports an outdated setup example.
pub(super) const EXAMPLE_OUTDATED: u8 = 3;

/// The library outputs in the artifact directory.
struct Libraries {
    /// Every library output.
    all: Vec<Library>,
    /// The newest modification time among them.
    newest: Option<SystemTime>,
}

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
    for entry in std::fs::read_dir(directory).map_err(|error| at(directory, error))? {
        let entry = entry.map_err(|error| at(directory, error))?;
        let kind = entry
            .file_type()
            .map_err(|error| at(&entry.path(), error))?;
        if kind.is_dir() {
            set_tree_time(&entry.path(), time)?;
        } else if kind.is_file() {
            set_modified(&entry.path(), time)?;
        }
    }
    Ok(())
}

/// Give the file at `path` the modification time `time` without opening it for writing.
///
/// macOS can kill the next launch of an executable that was opened for writing after it ran, and
/// Unix changes the times of a file its caller owns through a read-only handle. Windows changes
/// times only through a handle with write access.
///
/// # Errors
/// Returns a failed open or time change.
fn set_modified(path: &Path, time: SystemTime) -> io::Result<()> {
    let mut options = std::fs::File::options();
    options.read(cfg!(unix)).write(!cfg!(unix));
    options
        .open(path)
        .and_then(|file| file.set_modified(time))
        .map_err(|error| at(path, error))
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
    let mut found = false;
    let mut stale = false;
    let mut current: Vec<(PathBuf, SystemTime)> = Vec::new();
    for artifact in executables(directory)? {
        found = true;
        let modified = modified(&artifact)?;
        let outdated = !archived && outdated(&artifact, modified, &fingerprints, &libraries)?;
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
                set_modified(artifact, newest)?;
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
fn libraries(deps: &Path) -> io::Result<Libraries> {
    let mut all = Vec::new();
    let Ok(entries) = std::fs::read_dir(deps) else {
        return Ok(Libraries { all, newest: None });
    };
    for entry in entries {
        let path = entry.map_err(|error| at(deps, error))?.path();
        let extension = path.extension().and_then(|extension| extension.to_str());
        if path.is_file()
            && extension.is_some_and(|extension| LIBRARY_EXTENSIONS.contains(&extension))
        {
            all.push(Library {
                name: file_name(&path),
                modified: modified(&path)?,
            });
        }
    }
    let newest = all.iter().map(|library| library.modified).max();
    Ok(Libraries { all, newest })
}

/// The time `path` was last modified.
fn modified(path: &Path) -> io::Result<SystemTime> {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .map_err(|error| at(path, error))
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
    libraries: &Libraries,
) -> io::Result<bool> {
    let record = match executable_fingerprint(artifact, fingerprints)? {
        Some(record) => Some(record),
        None => named_fingerprint(artifact, fingerprints)?,
    };
    let Some(record) = record else {
        return Ok(libraries.newest.is_some_and(|newest| newest > modified));
    };
    let Some(linked) = linked_libraries(record, fingerprints)? else {
        return Ok(true);
    };
    Ok(libraries.all.iter().any(|library| {
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
    /// The fingerprints of build-script runs, whose results no executable links.
    scripts: HashSet<u64>,
}

/// The fingerprint JSON files Cargo has written under one profile directory.
fn fingerprint_files(directory: &Path) -> io::Result<Fingerprints> {
    let entries = match std::fs::read_dir(directory.join(".fingerprint")) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Fingerprints::default()),
        Err(error) => return Err(at(&directory.join(".fingerprint"), error)),
    };
    let mut files = Fingerprints::default();
    for entry in entries {
        let records = entry
            .map_err(|error| at(&directory.join(".fingerprint"), error))?
            .path();
        for file in std::fs::read_dir(&records).map_err(|error| at(&records, error))? {
            let path = file.map_err(|error| at(&records, error))?.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let name = file_name(&path);
            // Cargo names a build script's crate after the script's file, so only the record
            // kind identifies its run.
            if name.starts_with("run-") {
                files
                    .scripts
                    .extend(fingerprint_hash(&path.with_extension(""))?);
                continue;
            }
            if !name.starts_with("lib-") {
                files.executables.push(path);
                continue;
            }
            let Some(hash) = fingerprint_hash(&path.with_extension(""))? else {
                continue;
            };
            files.libraries.entry(hash).or_default().push(path);
        }
    }
    Ok(files)
}

/// Read a Cargo dependency record, refusing malformed records instead of guessing its inputs.
fn read_fingerprint(path: &Path) -> io::Result<Fingerprint> {
    let bytes = std::fs::read(path).map_err(|error| at(path, error))?;
    serde_json::from_slice(&bytes).map_err(|error| {
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
    let entries = match std::fs::read_dir(&folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(at(&folder, error)),
    };
    for entry in entries {
        let twin = entry.map_err(|error| at(&folder, error))?.path();
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

/// The record of an executable Cargo names without a hash, as it names Windows MSVC binaries and
/// examples: the one record of that target, or `None` when the target has none or several that
/// differ, since the name then no longer tells which build wrote the file.
fn named_fingerprint(
    artifact: &Path,
    fingerprints: &Fingerprints,
) -> io::Result<Option<Fingerprint>> {
    let stem = artifact
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let examples = artifact
        .parent()
        .is_some_and(|parent| parent.ends_with("examples"));
    let record = format!("{}-{stem}.json", if examples { "example" } else { "bin" });
    let mut found = None;
    for path in &fingerprints.executables {
        if file_name(path) != record {
            continue;
        }
        let next = read_fingerprint(path)?;
        if found.as_ref().is_some_and(|previous| *previous != next) {
            return Ok(None);
        }
        found = Some(next);
    }
    Ok(found)
}

/// Compare equal-sized artifacts without retaining their executable images in memory.
fn same_contents(left: &Path, right: &Path) -> io::Result<bool> {
    let open = |path: &Path| std::fs::File::open(path).map_err(|error| at(path, error));
    let (mut first, mut second) = (open(left)?, open(right)?);
    let mut remaining = first.metadata().map_err(|error| at(left, error))?.len();
    if remaining != second.metadata().map_err(|error| at(right, error))?.len() {
        return Ok(false);
    }
    let mut a = [0; 8192];
    let mut b = [0; 8192];
    while remaining > 0 {
        let count = usize::try_from(remaining.min(8192)).map_err(io::Error::other)?;
        first
            .read_exact(&mut a[..count])
            .map_err(|error| at(left, error))?;
        second
            .read_exact(&mut b[..count])
            .map_err(|error| at(right, error))?;
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
    while let Some((_, _, _, hash)) = pending.pop() {
        // Build-script results influence compilation but are not linked library outputs.
        if fingerprints.scripts.contains(&hash) || !seen.insert(hash) {
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

/// Decode Cargo's little-endian hexadecimal hash, or `None` for an invalidated build.
fn fingerprint_hash(path: &Path) -> io::Result<Option<u64>> {
    let text = std::fs::read_to_string(path).map_err(|error| at(path, error))?;
    // Cargo truncates the hash before rebuilding and fills it only after success.
    if text.is_empty() {
        return Ok(None);
    }
    if text.len() != 16 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid Cargo fingerprint hash {}", path.display()),
        ));
    }
    u64::from_str_radix(&text, 16)
        .map(|hash| Some(hash.swap_bytes()))
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid Cargo fingerprint hash {}: {error}", path.display()),
            )
        })
}
