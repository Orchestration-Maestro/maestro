//! Changelog sections and the numeric release order they carry.

/// A changelog section and the numeric version from its heading.
#[derive(Debug, Clone, PartialEq)]
pub struct ChangelogEntry {
    /// Major component.
    pub major: f64,
    /// Minor component.
    pub minor: f64,
    /// Patch component.
    pub patch: f64,
    /// The heading line followed by the section body.
    pub content: String,
}

/// Compares the first differing numeric component, retaining its difference.
///
/// The result is positive when `v1` is newer, negative when it is older and zero when
/// the numeric components match. Section content never decides the order.
#[must_use]
pub fn compare_versions(v1: &ChangelogEntry, v2: &ChangelogEntry) -> f64 {
    if !same(v1.major, v2.major) {
        return v1.major - v2.major;
    }
    if !same(v1.minor, v2.minor) {
        return v1.minor - v2.minor;
    }
    v1.patch - v2.patch
}

/// Whether two numeric components hold the same value, counting a missing value as different.
fn same(left: f64, right: f64) -> bool {
    left.partial_cmp(&right) == Some(std::cmp::Ordering::Equal)
}
