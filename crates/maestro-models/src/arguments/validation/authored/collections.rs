//! JSON pointer addressing and the pattern expressions record members use.

use regress::Regex;

/// Append a percent-agnostic JSON pointer segment, escaping the reference markers.
pub(super) fn escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// Address a child segment of the supplied pointer.
pub(super) fn child_pointer(pointer: &str, suffix: &str) -> String {
    format!("{pointer}/{suffix}")
}

/// Build the expression a declared pattern stands for, when it compiles.
pub(super) fn pattern(source: &str) -> Option<Regex> {
    Regex::with_flags(source, "u").ok()
}
