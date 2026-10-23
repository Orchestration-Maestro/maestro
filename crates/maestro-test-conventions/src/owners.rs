//! One owner for the session directory name.

use crate::source::{Member, test_layout};
use std::path::Path;

/// The crate that owns the encoder.
const OWNER_CRATE: &str = "maestro-session";
/// The encoder's own module.
const OWNER_FILE: &str = "src/session_manager/files.rs";
/// The published encoder name.
const ENCODER: &str = "encode_session_dir";
/// Every production file that must reach the encoder instead of spelling the name.
const CONSUMERS: [(&str, &str); 4] = [
    ("maestro-app", "src/developer_tools/cost.rs"),
    ("maestro-app", "src/developer_tools/stats.rs"),
    (
        "maestro-app",
        "src/developer_tools/session_transcripts/mod.rs",
    ),
    ("maestro-session", "src/session_manager/files.rs"),
];

/// Reject a second session directory encoder and require every selector to call the owner.
///
/// The check is structural: it reads production source lines, does not expand
/// macros and does not resolve re-exports or renamed imports.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut owned = false;
    for member in members {
        for source in &member.sources {
            let relative = relative_path(&source.path, &member.directory);
            if !relative.starts_with("src/") || test_layout(&source.path, &member.directory) {
                continue;
            }
            if member.name == OWNER_CRATE && relative == OWNER_FILE {
                definitions(&source.contents)?;
                owned = true;
            } else if let Some(line) = copy(&source.contents) {
                return Err(format!(
                    "{}:{line}: session directory encoding belongs to {OWNER_CRATE}::{ENCODER}",
                    source.path.display()
                ));
            }
        }
    }
    if !owned {
        return Ok(());
    }
    for (member, relative) in CONSUMERS {
        if !members.iter().any(|found| found.name == member) {
            continue;
        }
        let found = members
            .iter()
            .find(|found| found.name == member)
            .and_then(|found| {
                found
                    .sources
                    .iter()
                    .find(|source| relative_path(&source.path, &found.directory) == relative)
            })
            .ok_or_else(|| format!("{member}: missing session directory selector {relative}"))?;
        if !found.contents.contains(&format!("{ENCODER}(")) {
            return Err(format!(
                "{}: session directory selector does not call {ENCODER}",
                found.path.display()
            ));
        }
    }
    Ok(())
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

/// The crate-relative path of one source, spelled with `/`.
fn relative_path(path: &Path, directory: &Path) -> String {
    path.strip_prefix(directory)
        .unwrap_or(path)
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
