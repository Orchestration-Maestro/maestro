//! Unix file-system and Git process effects.

use std::fs;
use std::io;
use std::process::{Command, Stdio};

use super::operations::{FooterFileKind, FooterOperations};

/// Footer effects on the local file system and the `git` command.
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeFooterOperations;

impl FooterOperations for NativeFooterOperations {
    fn exists(&self, path: &str) -> bool {
        fs::exists(path).unwrap_or(false)
    }

    fn stat_kind(&self, path: &str) -> io::Result<FooterFileKind> {
        let kind = fs::metadata(path)?.file_type();
        Ok(match (kind.is_file(), kind.is_dir()) {
            (true, _) => FooterFileKind::File,
            (_, true) => FooterFileKind::Directory,
            _ => FooterFileKind::Other,
        })
    }

    fn read_text(&self, path: &str) -> io::Result<String> {
        Ok(String::from_utf8_lossy(&fs::read(path)?).into_owned())
    }

    fn current_dir(&self) -> io::Result<String> {
        Ok(std::env::current_dir()?.to_string_lossy().into_owned())
    }

    fn drive_directory(&self, _drive: char) -> Option<String> {
        None
    }

    fn symbolic_ref_sync(&self, repo_dir: &str) -> io::Result<Option<String>> {
        let output = Command::new("git")
            .arg("--no-optional-locks")
            .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
            .current_dir(repo_dir)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()?;
        Ok(output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).into_owned()))
    }
}
