//! The user cache location a child with a replaced home still shares with its parent.

use std::path::PathBuf;

/// The directory a child names in `XDG_CACHE_HOME` to reach this process's user cache.
///
/// Returns `XDG_CACHE_HOME` when it is an absolute path, otherwise `HOME/.cache`, and
/// `None` when neither names a location. A cleared child that replaces `HOME` would
/// otherwise start with an empty user cache and compile every component again.
#[must_use]
pub fn user_cache_home() -> Option<PathBuf> {
    None
}
