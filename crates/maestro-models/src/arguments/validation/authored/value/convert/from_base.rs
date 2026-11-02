//! Conversion supplied by an authored Base node.

use crate::DiagnosticErrorInfo;
use crate::arguments::validation::authored::{context::Context, schema_view::View};
use serde_json::Value;

/// Run the retained converter, preserving replacement separately from in-place edits.
pub(crate) fn from_base(
    converter: &Context<'_>,
    view: View<'_>,
    value: &mut Value,
) -> Result<Option<Value>, DiagnosticErrorInfo> {
    match converter
        .code
        .node(view.raw)
        .and_then(|node| node.base.as_ref())
    {
        Some(base) => base.convert.call(value),
        None => Ok(None),
    }
}
