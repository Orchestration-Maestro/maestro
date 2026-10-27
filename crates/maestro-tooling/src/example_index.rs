//! Render the extension index from each file's own documentation.

use std::ffi::OsString;
use std::fmt::Write;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Opening marker for the generated table.
const START: &str = "<!-- example-index:start -->";
/// Closing marker for the generated table.
const END: &str = "<!-- example-index:end -->";

/// Refresh the table, or check it without changing files.
pub fn run(args: &[OsString], checkout: &Path) -> io::Result<ExitCode> {
    if !args.is_empty() && args != [OsString::from("--check")] {
        return Err(io::Error::other("usage: example-index [--check]"));
    }
    let folder = checkout.join("examples/extensions");
    let readme = folder.join("README.md");
    let before = fs::read_to_string(&readme)?;
    let after = render(&folder, &before)?;
    if args.is_empty() {
        fs::write(readme, after)?;
    } else if before != after {
        return Err(io::Error::other(
            "extension index is stale; run just example-index",
        ));
    }
    Ok(ExitCode::SUCCESS)
}

/// Replace only the marked table, retaining all surrounding author guidance.
fn render(folder: &Path, before: &str) -> io::Result<String> {
    let (prefix, rest) = before
        .split_once(START)
        .ok_or_else(|| io::Error::other("missing example index start marker"))?;
    let (_, suffix) = rest
        .split_once(END)
        .ok_or_else(|| io::Error::other("missing example index end marker"))?;
    let mut files = Vec::new();
    collect(folder, &mut files)?;
    files.sort_by_cached_key(|path| path.to_string_lossy().replace('\\', "/"));
    let mut result = format!("{prefix}{START}\n| File | Provides |\n| --- | --- |\n");
    for file in files {
        let relative = file.strip_prefix(folder).map_err(io::Error::other)?;
        let name = relative.to_string_lossy().replace('\\', "/");
        let summary = summary(&file)?.replace('|', "\\|");
        writeln!(result, "| `{name}` | {summary} |").map_err(io::Error::other)?;
    }
    write!(result, "{END}{suffix}").map_err(io::Error::other)?;
    Ok(result)
}

/// Gather documented files recursively, including helpers and usage pages.
fn collect(folder: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect(&entry.path(), files)?;
        } else if kind.is_file() && entry.file_name() != "README.md" {
            files.push(entry.path());
        }
    }
    Ok(())
}

/// Read the initial Rust documentation paragraph or Markdown page heading.
fn summary(path: &Path) -> io::Result<String> {
    let text = fs::read_to_string(path)?;
    let result = if path.extension().is_some_and(|extension| extension == "rs") {
        text.lines()
            .map_while(|line| line.strip_prefix("//!"))
            .map(str::trim)
            .take_while(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        text.lines()
            .next()
            .and_then(|line| line.strip_prefix("# "))
            .unwrap_or_default()
            .to_owned()
    };
    if result.is_empty() {
        return Err(io::Error::other(format!(
            "{} needs opening documentation for the example index",
            path.display()
        )));
    }
    Ok(result)
}
