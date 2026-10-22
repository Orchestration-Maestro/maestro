//! Package-override precedence over the executable lookup of the native getters.

use super::{installation, package_dir, theme_directories};
use std::io;

/// An executable lookup that fails, as when the running executable cannot be located.
fn missing_executable() -> io::Result<std::path::PathBuf> {
    Err(io::Error::other("executable not found"))
}

#[test]
fn package_override_skips_the_executable_lookup() {
    let installation = || {
        let (module_dir, layout) = installation::probe_layout();
        installation::Installation::locate(module_dir, layout, missing_executable)
    };
    assert_eq!(package_dir(installation, Some("/pkg")).unwrap(), "/pkg");
    let error = package_dir(installation, None).unwrap_err();
    assert_eq!(error.to_string(), "executable not found");
}

// The asserted theme path carries Unix separators.
#[cfg(unix)]
#[test]
fn theme_override_skips_the_executable_lookup() {
    let directories = theme_directories(Some("/pkg"), missing_executable).unwrap();
    assert_eq!(
        directories.themes_dir,
        "/pkg/crates/maestro-theme/assets/theme"
    );
    let error = theme_directories(None, missing_executable).unwrap_err();
    assert_eq!(error.to_string(), "executable not found");
}
