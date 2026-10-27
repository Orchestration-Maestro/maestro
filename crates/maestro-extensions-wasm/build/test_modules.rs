//! Discover the shipped examples' controlled and component test modules.

use std::io;
use std::path::Path;

/// Render module declarations for the example test files in a supplied directory.
///
/// # Errors
/// Returns a directory or filename decoding failure.
pub fn generate(directory: &Path) -> io::Result<String> {
    std::fs::read_dir(directory)?;
    Ok(String::new())
}
