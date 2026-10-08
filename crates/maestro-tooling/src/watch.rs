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
    if let Some(ignore) = target_ignore(target.as_std_path(), &std::env::current_dir()?)? {
        command.args(["--ignore", &ignore]);
    }
    let status = command
        .args([
            "--watch",
            ".",
            "--ignore-nothing",
            "--ignore",
            "/.git/",
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

/// Anchor the literal output subtree beneath the watched directory.
fn target_ignore(target: &Path, checkout: &Path) -> io::Result<Option<String>> {
    let Ok(relative) = target.strip_prefix(checkout) else {
        return Ok(None);
    };
    let relative = relative
        .to_str()
        .ok_or_else(|| io::Error::other("non-Unicode target path"))?;
    let relative = relative.replace(std::path::MAIN_SEPARATOR, "/");
    let mut pattern = String::from("/");
    for character in relative.chars() {
        if matches!(character, '\\' | '[' | ']' | '*' | '?' | '!' | '#') {
            pattern.push('\\');
        }
        pattern.push(character);
    }
    pattern.push('/');
    Ok(Some(pattern))
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
