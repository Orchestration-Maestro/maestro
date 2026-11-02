//! Rendered values of prepared tool records, projected for native tests.
use super::renderers::{card_content::CardContent, collapsible_card, default_renderer};
use serde_json::Value;
use std::any::Any;

/// A built-in status, disclosure or default card's rendered values.
pub(crate) fn prepared(content: &dyn Any) -> Option<Value> {
    content
        .downcast_ref::<CardContent>()
        .map(CardContent::view)
        .or_else(|| collapsible_card::view::prepared(content))
        .or_else(|| default_renderer::view::prepared(content))
}
