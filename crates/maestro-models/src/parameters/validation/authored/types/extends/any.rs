//! Extends check for a left node that accepts every value.

/// Every right node accepts a value the left node already accepts.
///
/// The reference separates the inferred variants here; the authored JSON contract has no
/// inferred types, so both true-like results mean the same acceptance.
pub(crate) fn extends_any() -> bool {
    true
}
