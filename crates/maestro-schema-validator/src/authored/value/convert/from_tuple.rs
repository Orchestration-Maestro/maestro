//! Conversion through the declared positions of a sequence.

use maestro_interchange::diagnostics::DiagnosticErrorInfo;
use serde_json::Value;

use super::convert_member;
use crate::authored::context::Context;
use crate::authored::schema_view::{Node, View};
use crate::coercion;

/// Convert the leading positions of a sequence through the nodes an authored tuple declares.
pub(crate) fn from_tuple<'a>(
    converter: &mut Context<'a>,
    view: View<'a>,
    value: &mut Value,
) -> Result<Option<Value>, DiagnosticErrorInfo> {
    let Some(values) = value.as_array_mut() else {
        return Ok(None);
    };
    let mut result = Ok(());
    coercion::positions(view.raw, values, |member, item| {
        let item = Node::Leaf(converter.document.view(item));
        if result.is_ok() {
            result = convert_member(converter, &item, member);
        }
    });
    result?;
    Ok(None)
}
