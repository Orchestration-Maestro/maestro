//! Rejects handwritten JavaScript in crate sources.

use std::path::{Path, PathBuf};

/// Script extensions that mark a source file as handwritten runtime logic.
const EXTENSIONS: [&str; 3] = ["js", "mjs", "cjs"];

/// Fail when a script file sits anywhere below `crates/*/src`.
///
/// Maestro's in-frame runtime is Rust. Generated output belongs under a crate's
/// `assets` directory, and supplied test inputs stay beside their tests; neither
/// is crate source. A workspace without a `crates` directory has nothing to check.
pub(crate) fn check(root: &Path) -> Result<(), String> {
    let crates = root.join("crates");
    let entries = match std::fs::read_dir(&crates) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("{}: {error}", crates.display())),
    };
    for entry in entries {
        let source = entry
            .map_err(|error| format!("{}: {error}", crates.display()))?
            .path()
            .join("src");
        if let Some(script) = first_script(&source)? {
            return Err(format!(
                "{}: crate sources hold no handwritten JavaScript; runtime logic is Rust and generated output lives under assets/",
                script.display()
            ));
        }
    }
    Ok(())
}

/// The first script file below `directory`, in name order, if any.
fn first_script(directory: &Path) -> Result<Option<PathBuf>, String> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{}: {error}", directory.display())),
    };
    let mut paths = entries
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("{}: {error}", directory.display()))?;
    paths.sort();
    for path in paths {
        if path.is_dir() {
            if let Some(script) = first_script(&path)? {
                return Ok(Some(script));
            }
        } else if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| EXTENSIONS.contains(&extension))
        {
            return Ok(Some(path));
        }
    }
    Ok(None)
}
