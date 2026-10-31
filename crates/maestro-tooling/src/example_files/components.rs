//! Portable source discovery for component preparation and prepared-artifact lookup.

use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
};

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

/// The author and context exports that every preparation provides, so no marker may reuse them.
pub const FIXED_VARIABLES: [&str; 2] = ["MAESTRO_AUTHOR_COMPONENT", "MAESTRO_CONTEXT_COMPONENT"];

/// The author and context examples, which no source marker names, in the order of the fixed
/// exports that supply their artifacts.
pub const FIXED_EXAMPLES: [&str; 2] = ["author_component", "context_bridge"];

/// Visits marked components in source order, validating each export before its callback.
///
/// Variables are unique across all roots, including the fixed author and context exports.
///
/// # Errors
/// Returns discovery, invalid or duplicate marker, source-reading or callback failures.
pub fn visit(
    folders: &[PathBuf],
    mut visit: impl FnMut(&str, &str) -> io::Result<()>,
) -> io::Result<()> {
    let mut variables = HashSet::from(FIXED_VARIABLES.map(String::from));
    let mut files = Vec::new();
    for folder in folders {
        files.extend(collect(folder)?.into_iter().map(|file| (folder, file)));
    }
    for (folder, file) in files {
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
            let name = target_name(folder, &file)?;
            visit(&name, variable)?;
        }
    }
    Ok(())
}

/// Keep variable names within the component environment namespace and one output line.
///
/// # Errors
/// Returns an error naming a variable that is outside that namespace.
pub fn validate_variable(variable: &str) -> io::Result<()> {
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

/// Derive Cargo's target name from a source file or a directory entry point.
///
/// # Errors
/// Returns an error when the target name is absent or is not UTF-8.
pub fn target_name(folder: &Path, source: &Path) -> io::Result<String> {
    let directory =
        source.parent() != Some(folder) && source.file_name().is_some_and(|name| name == "main.rs");
    let name = if directory {
        source.parent().and_then(Path::file_name)
    } else {
        source.file_stem()
    }
    .and_then(|name| name.to_str())
    .ok_or_else(|| io::Error::other("component target needs a UTF-8 name"))?;
    Ok(if directory {
        name.replace('-', "_")
    } else {
        name.to_owned()
    })
}
