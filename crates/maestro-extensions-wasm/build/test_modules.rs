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
    // The expansion keeps ordinary module ownership, including nested-file lookup.
    let mut result = String::from(
        "/// Declare discovered modules with their ordinary source directories.\nmacro_rules! example_tests { () => {\n",
    );
    for file in files {
        let name = file
            .file_stem()
            .and_then(|name| name.to_str())
            .ok_or_else(|| io::Error::other("example test name is not UTF-8"))?;
        writeln!(
            result,
            "/// Controlled and component checks for `{name}`.\nmod {name};"
        )
        .map_err(io::Error::other)?;
    }
    result.push_str("} }\n");
    Ok(result)
}
