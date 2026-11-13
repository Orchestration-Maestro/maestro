//! Resource path resolution, classification and real-path fallback.

use crate::{catalog::real_path, metadata::text_whitespace};
#[doc(inline)]
pub use maestro_path::fs::{resolve, resolve_groups, resolve_groups_with};
use std::{
    convert::Infallible,
    io,
    path::{Path, PathBuf},
};

/// The project and user configuration directory name.
pub const CONFIG_DIR_NAME: &str = ".maestro";

/// The home directory as text.
pub use maestro_path::home_directory;

/// Classify local paths, excluding the known lowercase package and URL prefixes.
pub fn is_local_path(value: &str) -> bool {
    let value = value.trim_matches(text_whitespace);
    !["npm:", "git:", "github:", "http:", "https:", "ssh:"]
        .iter()
        .any(|prefix| value.starts_with(prefix))
}

/// Resolve an explicit instruction path: trim it, expand a leading `~` below `home`, keep an
/// absolute result and resolve any other result against `cwd`.
///
/// # Errors
/// Returns the working-directory error of a resolution that needed it.
pub(crate) fn resolve_explicit_path(path: &str, cwd: &str, home: &str) -> io::Result<String> {
    let path = path.trim_matches(text_whitespace);
    let Ok(normalized) =
        maestro_path::expand_tilde_joined(path, || Ok::<_, Infallible>(home.to_owned()));
    if maestro_path::is_absolute(&normalized) {
        Ok(normalized)
    } else {
        resolve(&[cwd, &normalized])
    }
}

/// The package home: a nonempty `HOME` on any platform, otherwise the native home.
///
/// The native fallback preserves an empty Unix `HOME`; other resource callers keep
/// the rule of [`home_directory`].
///
/// # Errors
/// Returns a not-found error for an unknown home, or invalid-data for non-UTF-8 bytes.
pub fn package_home_directory() -> io::Result<String> {
    maestro_path::home_directory_text(
        std::env::var_os("HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(maestro_path::native_home_dir),
    )
}

/// Compare preserved target spelling against a root resolved from the process
/// directories, on a separator boundary: `root` itself or any path below it.
///
/// # Errors
/// Returns the working directory's error when resolving `root` needs it.
pub fn is_under_path(target: &str, root: &str) -> io::Result<bool> {
    let root = resolve(&[root])?;
    let sep = maestro_path::SEP;
    Ok(target == root || target.starts_with(&format!("{}{sep}", root.trim_end_matches(sep))))
}

/// Return the real path or retain the original spelling when resolving it fails.
#[must_use]
pub fn canonicalize_path(path: &str) -> String {
    real_path(Path::new(path)).map_or_else(
        |_| path.to_owned(),
        |resolved| resolved.to_string_lossy().into_owned(),
    )
}
