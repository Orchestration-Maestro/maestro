//! Expandable literal summary with children captured once by their owner.
use crate::page::PreparedContent;

/// Disclosure with a literal summary over children captured once at construction.
pub struct ExpandableSection {
    /// Caller-authored literal summary.
    pub summary: String,
    /// Expansion applied when a root mounts or reconnects.
    pub default_expanded: bool,
    /// Original children, never recaptured from generated UI.
    children: Vec<PreparedContent>,
}
impl ExpandableSection {
    /// Capture the original owned children once.
    #[must_use]
    pub fn new(children: Vec<PreparedContent>) -> Self {
        Self {
            summary: String::new(),
            default_expanded: false,
            children,
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod browser;

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests;
/// Rendered-value projections for native tests.
#[cfg(all(not(target_arch = "wasm32"), any(test, feature = "test-support")))]
pub(crate) mod view;
