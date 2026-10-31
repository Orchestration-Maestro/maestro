//! Gives the executables of an artifact directory one modification time.
//!
//! The pinned engine's cache namespace includes the modification time of the executable that
//! consumes it, so the setup example and the test binaries must share a time. The installed
//! browser graph shares one time as well, so Cargo finds no output newer than what builds on it.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::SystemTime;

use super::nonempty;
use crate::cargo_lock::Lock;
use crate::dep_info;
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
/// Returns a directory without executables or a failed read or write.
pub(super) fn run(checkout: &Path, directory: &Path, archived: bool) -> io::Result<ExitCode> {
    Ok(match normalize(checkout, directory, archived)? {
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
/// Returns a directory without executables or a failed read or write.
pub(super) fn normalize(checkout: &Path, directory: &Path, archived: bool) -> io::Result<Outcome> {
    let crates = workspace_closures(checkout);
    let libraries = libraries(&directory.join("deps"))?;
    let newest_library = libraries.iter().map(|library| library.modified).max();
    let mut found = false;
    let mut stale = false;
    let mut current: Vec<(PathBuf, SystemTime)> = Vec::new();
    for artifact in executables(directory)? {
        found = true;
        let modified = std::fs::metadata(&artifact)?.modified()?;
        let outdated =
            !archived && outdated(&artifact, modified, &crates, &libraries, newest_library);
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

/// For each workspace crate, the underscored names of it and every workspace crate it builds
/// on. Registry crates are left out: a change in one reaches its dependents through the
/// fingerprint hash, so their output times do not matter. A lock file that cannot be read
/// leaves every executable to the newest-library rule.
fn workspace_closures(checkout: &Path) -> HashMap<String, Vec<String>> {
    let Ok(lock) = Lock::read(checkout) else {
        return HashMap::new();
    };
    let graph = lock.graph(|package| package.source.is_none());
    lock.packages()
        .iter()
        .filter(|package| package.source.is_none())
        .map(|package| {
            let mut built = graph
                .closure([package.name.as_str()])
                .into_iter()
                .map(|member| member.name.replace('-', "_"))
                .collect::<Vec<_>>();
            built.sort();
            (package.name.clone(), built)
        })
        .collect()
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

/// Whether a library output that `artifact` builds on is newer than it.
///
/// The libraries come from the crate named in the dependency information Cargo left beside
/// the executable and the workspace crates the lock file lists for it. A library's own test
/// binary is built from the crate and not on its output. Without that information, any newer
/// library output outdates the executable.
fn outdated(
    artifact: &Path,
    modified: SystemTime,
    crates: &HashMap<String, Vec<String>>,
    libraries: &[Library],
    newest_library: Option<SystemTime>,
) -> bool {
    let source = dependency_source(artifact);
    let package = source
        .strip_prefix("crates/")
        .unwrap_or(&source)
        .split('/')
        .next()
        .unwrap_or_default();
    let Some(closure) = crates.get(package) else {
        return newest_library.is_some_and(|newest| newest > modified);
    };
    let own = package.replace('-', "_");
    let own_library = format!("crates/{package}/src/lib.rs");
    closure
        .iter()
        .filter(|member| !(**member == own && source == own_library))
        .any(|member| {
            let prefix = format!("lib{member}-");
            libraries
                .iter()
                .any(|library| library.name.starts_with(&prefix) && library.modified > modified)
        })
}

/// The first source file of the dependency information Cargo wrote beside `artifact`.
fn dependency_source(artifact: &Path) -> String {
    let mut information = artifact.as_os_str().to_owned();
    information.push(".d");
    let text = std::fs::read_to_string(information).unwrap_or_default();
    dep_info::sources(&text)
        .and_then(|mut sources| sources.next())
        .unwrap_or_default()
        .to_owned()
}
