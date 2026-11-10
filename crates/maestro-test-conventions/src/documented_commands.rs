//! Rejects a program in the reader's home directory in documented commands.

use std::path::{Path, PathBuf};

/// Directories under a reader's home that hold programs a checkout does not provide.
const HOME_PROGRAMS: [&str; 4] = ["/.local/bin/", "~/bin/", "$HOME/bin/", "${HOME}/bin/"];

/// Directories that hold no documentation.
const SKIPPED: [&str; 2] = [".git", "target"];

/// Fail when Markdown tells a reader to run a program from their home directory.
///
/// A program there is a directory followed by its name, so a page that names the
/// directory itself stays accepted. Every Markdown file in the checkout counts,
/// documentation test fixtures included. Each offending file is reported once, at
/// its first such line. Build and version-control directories are skipped, and
/// files are reported in name order so the first diagnostic is stable.
pub(crate) fn check(root: &Path) -> Result<(), String> {
    let mut pages = Vec::new();
    markdown_pages(root, &mut pages)?;
    pages.sort();
    let mut offenders = Vec::new();
    for page in pages {
        let text = std::fs::read_to_string(&page)
            .map_err(|error| format!("{}: {error}", page.display()))?;
        if let Some(line) = first_offender(&text) {
            offenders.push(format!(
                "{}:{line}: documented commands run cargo or just, not a program from a home directory",
                page.display()
            ));
        }
    }
    if offenders.is_empty() {
        return Ok(());
    }
    Err(offenders.join("\n"))
}

/// Collect every Markdown file below `directory`.
fn markdown_pages(directory: &Path, pages: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(directory)
        .map_err(|error| format!("{}: {error}", directory.display()))?;
    for entry in entries {
        let path = entry
            .map_err(|error| format!("{}: {error}", directory.display()))?
            .path();
        if path.is_dir() {
            if !is_skipped(&path) {
                markdown_pages(&path, pages)?;
            }
        } else if path.extension().is_some_and(|extension| extension == "md") {
            pages.push(path);
        }
    }
    Ok(())
}

/// Whether `path` is a build or version-control directory.
fn is_skipped(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| SKIPPED.contains(&name))
}

/// The one-based number of the first line naming a home-directory program.
fn first_offender(text: &str) -> Option<usize> {
    text.lines()
        .position(|line| {
            HOME_PROGRAMS
                .iter()
                .any(|program| names_program(line, program))
        })
        .map(|index| index + 1)
}

/// Whether `line` names a program in the home directory spelled `program`.
fn names_program(line: &str, program: &str) -> bool {
    line.match_indices(program).any(|(start, _)| {
        line[start + program.len()..]
            .chars()
            .next()
            .is_some_and(|first| first.is_alphanumeric() || matches!(first, '_' | '.' | '-'))
    })
}
