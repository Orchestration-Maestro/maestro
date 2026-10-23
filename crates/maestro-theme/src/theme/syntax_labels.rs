//! Language labels the syntax registry admits.

/// Lowercase labels the registry names, its aliases included, one per line.
const LABELS: &str = include_str!("../../assets/syntax-labels.txt");

/// Whether `label` names a language the registry admits, compared without case.
#[must_use]
pub(crate) fn admits(label: &str) -> bool {
    LABELS
        .lines()
        .any(|known| known.eq_ignore_ascii_case(label))
}
