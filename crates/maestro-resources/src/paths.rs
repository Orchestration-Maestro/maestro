//! Resource path classification and canonicalization fallback.

use crate::{frontmatter::text_whitespace, skills::ResourceOperations};
use std::path::Path;

/// Classify local paths, excluding the known lowercase package and URL prefixes.
pub fn is_local_path(value: &str) -> bool {
    let value = value.trim_matches(text_whitespace);
    !["npm:", "git:", "github:", "http:", "https:", "ssh:"]
        .iter()
        .any(|prefix| value.starts_with(prefix))
}

/// Return a canonical path or retain the original spelling on failure.
pub fn canonicalize_path(path: &str, operations: &dyn ResourceOperations) -> String {
    operations.canonicalize(Path::new(path)).map_or_else(
        |_| path.to_owned(),
        |resolved| resolved.to_string_lossy().into_owned(),
    )
}
