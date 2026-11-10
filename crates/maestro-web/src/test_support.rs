//! Rendered-value projections of prepared page content for native tests.
use serde_json::Value;
use std::any::Any;

/// Project a prepared built-in card, default card, line difference or section to the values the
/// browser renders from it; `None` for caller-owned content.
#[must_use]
pub fn prepared_view(content: &dyn Any) -> Option<Value> {
    crate::tools::view::prepared(content)
        .or_else(|| crate::components::changes::view::prepared(content))
        .or_else(|| crate::components::expandable_section::view::prepared(content))
}
