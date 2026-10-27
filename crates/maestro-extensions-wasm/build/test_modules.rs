//! Discover the shipped examples' controlled and component test modules.

use std::fmt::Write;
use std::io;
use std::path::Path;

/// Render module declarations for the example test files in a supplied directory.
///
/// # Errors
/// Returns a directory or filename decoding failure.
pub fn generate(directory: &Path) -> io::Result<String> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry.file_name().to_string_lossy().ends_with("_example.rs")
        {
            files.push(entry.path());
        }
    }
    files.sort();
    let mut result = String::new();
    for file in files {
        let name = file
            .file_stem()
            .and_then(|name| name.to_str())
            .ok_or_else(|| io::Error::other("example test name is not UTF-8"))?;
        let path = file
            .to_str()
            .ok_or_else(|| io::Error::other("example test path is not UTF-8"))?;
        writeln!(
            result,
            "/// Controlled and component checks for `{name}`.\n#[path = {path:?}]\nmod {name};"
        )
        .map_err(io::Error::other)?;
    }
    Ok(result)
}
