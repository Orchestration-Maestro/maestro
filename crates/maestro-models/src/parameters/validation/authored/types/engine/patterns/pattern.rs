//! Pattern compilation for declared record members and template literals.

use regress::Regex;

/// Compile a declared pattern with Unicode semantics.
pub(crate) fn pattern(source: &str) -> Option<Regex> {
    Regex::with_flags(source, "u").ok()
}
