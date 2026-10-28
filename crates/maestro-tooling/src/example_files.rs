//! Source discovery shared by the extension index and component preparation.

use std::collections::HashSet;
use std::fmt::Write;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Gather documented files recursively in portable path order, excluding index pages.
///
/// # Errors
/// Returns directory-read failures.
pub fn collect(folder: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_into(folder, &mut files)?;
    files.sort_by_cached_key(|path| path.to_string_lossy().replace('\\', "/"));
    Ok(files)
}

/// Visit regular files without following symbolic links.
fn collect_into(folder: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect_into(&entry.path(), files)?;
        } else if kind.is_file() && entry.file_name() != "README.md" {
            files.push(entry.path());
        }
    }
    Ok(())
}

/// Build marked components and render their nextest environment entries.
///
/// A marker names the environment variable; the Rust file stem names the Cargo target.
///
/// # Errors
/// Returns discovery, invalid or duplicate marker, source-reading or build failures.
pub fn prepare(
    folder: &Path,
    mut build: impl FnMut(&str, &str) -> io::Result<PathBuf>,
) -> io::Result<String> {
    let mut result = String::new();
    let mut variables = HashSet::from([
        String::from("MAESTRO_AUTHOR_COMPONENT"),
        String::from("MAESTRO_CONTEXT_COMPONENT"),
    ]);
    for file in collect(folder)? {
        if file.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }
        let source = fs::read_to_string(&file)?;
        for variable in source
            .lines()
            .filter_map(|line| line.strip_prefix("// maestro-component: "))
        {
            validate_variable(variable)?;
            if !variables.insert(variable.to_owned()) {
                return Err(io::Error::other(format!(
                    "duplicate component variable: {variable}"
                )));
            }
            let name = file
                .file_stem()
                .and_then(|name| name.to_str())
                .ok_or_else(|| io::Error::other("component target needs a UTF-8 file stem"))?;
            let path = build(name, variable)?;
            writeln!(result, "{variable}={}", path.display()).map_err(io::Error::other)?;
        }
    }
    Ok(result)
}

/// Keep variable names within the component environment namespace and one output line.
fn validate_variable(variable: &str) -> io::Result<()> {
    if variable.starts_with("MAESTRO_")
        && variable.ends_with("_COMPONENT")
        && variable
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "invalid component variable: {variable}"
        )))
    }
}
