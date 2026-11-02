//! Rendered values of a section's captured children, projected for native tests.
use super::ExpandableSection;
use serde_json::{Value, json};
use std::any::Any;

/// Summary, initial expansion and each captured child's own projection.
pub(crate) fn prepared(content: &dyn Any) -> Option<Value> {
    content.downcast_ref::<ExpandableSection>().map(|section| {
        json!({
            "summary": section.summary,
            "expanded": section.default_expanded,
            "children": section
                .children
                .iter()
                .map(|child| crate::test_support::prepared_view(child.as_ref()))
                .collect::<Vec<_>>(),
        })
    })
}
