//! One owner for the session directory name.

use crate::source::{Member, test_layout};
use std::path::Path;

/// The crate that owns the encoder.
const OWNER_CRATE: &str = "maestro-session";
/// The encoder's own module.
const OWNER_FILE: &str = "src/session_manager/files.rs";
/// The encoder's name, which no other production file may spell.
const ENCODER: &str = "encode_session_dir";

/// Require one definition of the session directory encoder and no local copy of its name.
///
/// The encoder is private to its own crate, so a caller cannot build a session
/// directory name by calling it; what remains is a second definition and a local
/// spelling of the encoded name, both of which this check rejects.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    for member in members {
        for source in &member.sources {
            let relative = relative_path(&source.path, &member.directory);
            if !relative.starts_with("src/") || test_layout(&source.path, &member.directory) {
                continue;
            }
            if member.name == OWNER_CRATE && relative == OWNER_FILE {
                definitions(&source.contents)?;
            } else if let Some(line) = copy(&source.contents) {
                return Err(format!(
                    "{}:{line}: session directory encoding belongs to {OWNER_CRATE}::{ENCODER}",
                    source.path.display()
                ));
            }
        }
    }
    Ok(())
}

/// Require one definition of the published encoder in its own module.
fn definitions(contents: &str) -> Result<(), String> {
    let definitions = contents
        .lines()
        .filter(|line| line.contains(&format!("fn {ENCODER}(")))
        .count();
    if definitions == 1 {
        Ok(())
    } else {
        Err(format!(
            "{OWNER_CRATE}::{ENCODER} must have exactly one definition, found {definitions}"
        ))
    }
}

/// The line of the first local directory-name copy, if the file holds one.
fn copy(contents: &str) -> Option<usize> {
    for (index, line) in contents.lines().enumerate() {
        let replaces = line.contains(".replace(") || line.contains(".replacen(");
        let separators = line.contains("'/'") || line.contains("\"/\"");
        let dashes = line.contains("'-'") || line.contains("\"-\"");
        if replaces && separators && dashes {
            return Some(index + 1);
        }
        if line.contains("format!(\"--") && line.contains("--\"") {
            return Some(index + 1);
        }
    }
    None
}

/// The crate-relative path of one source, spelled with `/`.
fn relative_path(path: &Path, directory: &Path) -> String {
    path.strip_prefix(directory)
        .unwrap_or(path)
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
