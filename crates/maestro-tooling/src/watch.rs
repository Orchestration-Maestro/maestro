use std::ffi::OsString;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde::Deserialize;

/// Launch a source watcher that queues compilation after file changes.
pub(crate) fn run(args: &[OsString], cargo: &Path) -> io::Result<ExitCode> {
    let target = cargo_metadata::MetadataCommand::new()
        .cargo_path(cargo)
        .no_deps()
        .exec()
        .map_err(io::Error::other)?
        .target_directory;
    let mut command = Command::new("watchexec");
    let entries = std::fs::read_dir(std::env::current_dir()?)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<Vec<_>>>()?;
    for root in watch_roots(entries, target.as_std_path()) {
        command.arg("--watch").arg(root);
    }
    let status = command
        .args([
            "--ignore-nothing",
            "--on-busy-update=queue",
            "--shell=none",
            "--emit-events-to=json-stdio",
            "--",
        ])
        .arg(std::env::current_exe()?)
        .args(["watch-step", "--"])
        .args(args)
        .status()?;
    Ok(super::exit_code(status))
}

/// Select literal top-level paths, excluding Git metadata and the current output root.
fn watch_roots(entries: impl IntoIterator<Item = PathBuf>, target: &Path) -> Vec<PathBuf> {
    entries
        .into_iter()
        .filter(|entry| entry != target && entry.file_name() != Some(std::ffi::OsStr::new(".git")))
        .collect()
}

/// Compile on source changes while ignoring events confined to Cargo outputs.
pub(crate) fn step(args: &[OsString], cargo: &Path) -> io::Result<ExitCode> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .cargo_path(cargo)
        .no_deps()
        .exec()
        .map_err(io::Error::other)?;
    let target = metadata.target_directory.as_std_path();
    if !should_build(io::stdin().lock(), target)? {
        return Ok(ExitCode::SUCCESS);
    }
    let args = args.strip_prefix(&[OsString::from("--")]).unwrap_or(args);
    Ok(super::exit_code(Command::new(cargo).args(args).status()?))
}

/// Decide whether a batch contains source changes; pathless events compile too.
fn should_build(events: impl BufRead, target: &Path) -> io::Result<bool> {
    let mut paths = Vec::new();
    for line in events.lines() {
        let event: Event = serde_json::from_str(&line?).map_err(io::Error::other)?;
        paths.extend(event.tags.into_iter().filter_map(|tag| match tag {
            Tag::Path { absolute } => Some(absolute),
            Tag::Other => None,
        }));
    }
    Ok(paths.is_empty() || paths.iter().any(|path| !path.starts_with(target)))
}

#[derive(Deserialize)]
/// File-watcher event decoded from standard input.
struct Event {
    /// Event annotations used to find changed paths.
    tags: Vec<Tag>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
/// Supported watcher annotations with a fallback for unrelated kinds.
enum Tag {
    /// Filesystem path affected by the event.
    Path {
        /// Absolute changed path used to exclude build outputs.
        absolute: PathBuf,
    },
    #[serde(other)]
    /// Annotation unrelated to a filesystem path.
    Other,
}

#[cfg(test)]
mod tests;
