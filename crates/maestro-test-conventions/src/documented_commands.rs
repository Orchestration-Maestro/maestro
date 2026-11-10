//! Rejects a program in the reader's home directory in documented commands.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Directories under a reader's home that hold programs a checkout does not provide.
const HOME_PROGRAMS: [&str; 4] = ["/.local/bin/", "~/bin/", "$HOME/bin/", "${HOME}/bin/"];

/// Directories that hold no documentation.
const SKIPPED: [&str; 2] = [".git", "target"];

/// Fail when Markdown tells a reader to run a program from their home directory.
///
/// A program there is a directory followed by its name, so a page that names the
/// directory itself stays accepted. Version control decides which pages count, so a
/// page that never ships cannot fail the check. Each offending file is reported once,
/// at its first such line, in name order so the first diagnostic is stable.
pub(crate) fn check(root: &Path) -> Result<(), String> {
    let mut pages = tracked_pages(root)?;
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

/// Collect the Markdown files version control tracks below `root`.
///
/// A tree without version-control metadata holds only documentation, so a fixture
/// counts every page it holds.
fn tracked_pages(root: &Path) -> Result<Vec<PathBuf>, String> {
    let listing = Command::new("git")
        .args(["ls-files", "-z", "--", "*.md"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("{}: git {error}", root.display()))?;
    if !listing.status.success() {
        let mut pages = Vec::new();
        markdown_pages(root, &mut pages)?;
        return Ok(pages);
    }
    Ok(listing
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .filter_map(|path| std::str::from_utf8(path).ok())
        .map(|path| root.join(path))
        .collect())
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
