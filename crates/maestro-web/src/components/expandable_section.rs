//! Expandable literal summary with children captured once by their owner.
use crate::dom::PreparedContent;

/// Disclosure with a literal summary over children captured once at construction.
#[cfg_attr(
    not(target_arch = "wasm32"),
    allow(
        dead_code,
        reason = "native targets prepare records only the browser mounts"
    )
)]
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
