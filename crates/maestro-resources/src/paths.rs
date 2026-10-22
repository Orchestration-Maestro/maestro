//! Resource path classification and real-path fallback.

use crate::{frontmatter::text_whitespace, skills::real_path};
use std::path::Path;

/// The project and user configuration directory name.
pub const CONFIG_DIR_NAME: &str = ".maestro";

/// Classify local paths, excluding the known lowercase package and URL prefixes.
pub fn is_local_path(value: &str) -> bool {
    let value = value.trim_matches(text_whitespace);
    !["npm:", "git:", "github:", "http:", "https:", "ssh:"]
        .iter()
        .any(|prefix| value.starts_with(prefix))
}

/// Return the real path or retain the original spelling when resolving it fails.
#[must_use]
pub fn canonicalize_path(path: &str) -> String {
    real_path(Path::new(path)).map_or_else(
        |_| path.to_owned(),
        |resolved| resolved.to_string_lossy().into_owned(),
    )
}
