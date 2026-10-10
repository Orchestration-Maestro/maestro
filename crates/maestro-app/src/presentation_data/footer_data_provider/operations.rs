//! The file-system and process effects footer metadata needs.

use std::io;

/// What a path names after following symbolic links.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FooterFileKind {
    /// A regular file.
    File,
    /// A directory.
    Directory,
    /// Anything else, such as a socket or device.
    Other,
}

/// Effects behind footer metadata, supplied by the caller.
///
/// Implementations report effects only; branch selection and text trimming
/// stay with the provider.
pub trait FooterOperations {
    /// Whether `path` exists; an error while checking counts as absent.
    fn exists(&self, path: &str) -> bool;
    /// The kind of `path`, following symbolic links.
    ///
    /// # Errors
    /// Returns the failure to read the path's metadata.
    fn stat_kind(&self, path: &str) -> io::Result<FooterFileKind>;
    /// The text of the file at `path`, with invalid UTF-8 replaced by U+FFFD.
    ///
    /// # Errors
    /// Returns the failure to read the file.
    fn read_text(&self, path: &str) -> io::Result<String>;
    /// The process working directory, read only when a relative path needs it.
    ///
    /// # Errors
    /// Returns the failure to read the working directory.
    fn current_dir(&self) -> io::Result<String>;
    /// The current directory of a Windows drive, when one is recorded.
    fn drive_directory(&self, drive: char) -> Option<String>;
    /// Run `git --no-optional-locks symbolic-ref --quiet --short HEAD` in
    /// `repo_dir` and return its untrimmed standard output.
    ///
    /// A process that does not exit successfully gives `Ok(None)`.
    ///
    /// # Errors
    /// Returns the failure to start the process.
    fn symbolic_ref_sync(&self, repo_dir: &str) -> io::Result<Option<String>>;
}
