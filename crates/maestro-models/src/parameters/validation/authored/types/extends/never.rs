//! Extends check for a left node that accepts no value.

/// A node accepting no value extends every right node.
pub(crate) fn extends_never() -> bool {
    true
}
