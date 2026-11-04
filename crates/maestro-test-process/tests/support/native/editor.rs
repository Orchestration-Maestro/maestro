//! Controlled editor selected by a sibling JSON record.

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};

use maestro_test_process::child::leave;
use serde::Deserialize;

/// The only signal a controlled editor ends by.
#[derive(Deserialize)]
pub(super) enum ExitSignal {
    /// Terminate with the native signal rather than an exit code.
    #[serde(rename = "SIGTERM")]
    Terminate,
}

/// Edit effects parsed completely before any file is read or written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    /// Destination for the program's own name and its literal operands.
    argv: Option<String>,
    /// Environment variable holding the destination for the edited filename.
    result: Option<String>,
    /// Destination for a copy of the edited file as it was before the edit, keeping its mode.
    before: Option<String>,
    /// Destination for the three standard descriptors' identities, read on Linux.
    descriptors: Option<String>,
    /// New file bytes.
    content: Option<Vec<u8>>,
    /// Destination for a mode-preserving copy of the edited file.
    copy: Option<String>,
    /// Remove the edited file instead of writing `content`.
    #[serde(default)]
    remove: bool,
    /// Replace the written file with a directory.
    #[serde(default)]
    directory: bool,
    /// Make the edited file's directory read-only after the edit.
    #[serde(default)]
    read_only_parent: bool,
    /// Exit status when no signal is requested.
    #[serde(default)]
    code: u8,
    /// Native termination instead of a numeric exit.
    signal: Option<ExitSignal>,
}

impl Record {
    /// Require the family and supported effects before the first write.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let json = text
            .strip_prefix("editor\n")
            .ok_or_else(|| io::Error::other("missing editor family"))?;
        let record: Self = serde_json::from_str(json).map_err(io::Error::other)?;
        if (!cfg!(unix) && (record.read_only_parent || record.signal.is_some()))
            || (!cfg!(target_os = "linux") && record.descriptors.is_some())
        {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "editor effect is unavailable on this platform",
            ));
        }
        Ok(record)
    }
}

/// Write the program's own name and its operands, each ending at its native NUL.
fn argv(path: &str, operands: &[OsString]) -> io::Result<()> {
    let mut fields = Vec::new();
    let program = std::env::args_os()
        .next()
        .ok_or_else(|| io::Error::other("missing editor identity"))?;
    for operand in std::iter::once(program).chain(operands.iter().cloned()) {
        fields.extend_from_slice(operand.as_encoded_bytes());
        fields.push(0);
    }
    std::fs::write(path, fields)
}

/// Skip the editor's own option and report its first file through a named variable.
fn report(variable: &str, operands: &[OsString]) -> io::Result<()> {
    let mut operands = operands.iter();
    let mut edited = operands.next();
    if edited.is_some_and(|operand| operand == OsStr::new("--custom")) {
        edited = operands.next();
    }
    let edited = edited.ok_or_else(|| io::Error::other("no file to edit"))?;
    let result = std::env::var_os(variable)
        .ok_or_else(|| io::Error::other(format!("{variable} is unset")))?;
    std::fs::write(result, [edited.as_encoded_bytes(), b"\n"].concat())
}

/// Write each standard descriptor's device, inode, mode and device number.
#[cfg(target_os = "linux")]
fn descriptors(path: &str) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    let mut lines = String::new();
    for descriptor in 0..3 {
        let target = format!("/proc/self/fd/{descriptor}");
        let meta = std::fs::metadata(&target)?;
        let line = format!(
            "{} {} {:x} {}\n",
            meta.dev(),
            meta.ino(),
            meta.mode(),
            meta.rdev()
        );
        lines.push_str(&line);
    }
    std::fs::write(path, lines)
}

/// Report that this platform records no descriptor identity.
#[cfg(not(target_os = "linux"))]
fn descriptors(_path: &str) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "descriptor observation requires Linux proc files",
    ))
}

/// Apply the record's file effects in the order an editing program reaches them.
fn edit(file: &Path, record: &Record) -> io::Result<()> {
    if let Some(content) = &record.content {
        std::fs::write(file, content)?;
    }
    if record.directory {
        std::fs::remove_file(file)?;
        std::fs::create_dir(file)?;
    }
    if record.remove {
        std::fs::remove_file(file)?;
    }
    if let Some(copy) = &record.copy {
        std::fs::copy(file, copy)?;
    }
    if record.read_only_parent {
        read_only(file)?;
    }
    Ok(())
}

/// Make the edited file's directory read-only, keeping the caller's mode observation.
#[cfg(unix)]
fn read_only(file: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let parent = file
        .parent()
        .ok_or_else(|| io::Error::other("the edited file has no directory"))?;
    std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o500))
}

/// Report that this platform cannot set Unix directory permissions.
#[cfg(not(unix))]
fn read_only(_file: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "read-only editor directory requires Unix permissions",
    ))
}

/// End the editor by its native termination signal.
#[cfg(unix)]
fn terminate() -> io::Result<()> {
    use rustix::process::{Signal, getpid, kill_process};
    kill_process(getpid(), Signal::TERM).map_err(Into::into)
}

/// Report that this platform cannot deliver a Unix termination signal.
#[cfg(not(unix))]
fn terminate() -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "editor termination signal requires Unix",
    ))
}

/// The edited file's last operand, which every caller supplies.
fn target(operands: &[OsString]) -> io::Result<PathBuf> {
    operands
        .last()
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("missing edited file"))
}

/// Observe the edit, then end by the recorded status or signal.
pub(super) fn run(record: &Record, operands: &[OsString]) -> io::Result<()> {
    let file = target(operands)?;
    if let Some(variable) = &record.result {
        report(variable, operands)?;
    }
    if let Some(path) = &record.argv {
        argv(path, operands)?;
    }
    if let Some(path) = &record.before {
        std::fs::copy(&file, path)?;
    }
    if let Some(path) = &record.descriptors {
        descriptors(path)?;
    }
    edit(&file, record)?;
    if record.signal.is_some() {
        terminate()?;
    }
    leave(i32::from(record.code))
}
