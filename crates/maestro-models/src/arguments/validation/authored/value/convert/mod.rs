//! Conversion entries, one per authored kind and stage.
mod from_additional;
mod from_array;
mod from_base;
mod from_boolean;
mod from_cyclic;
mod from_enum;
mod from_integer;
mod from_intersect;
mod from_literal;
mod from_null;
mod from_number;
mod from_object;
mod from_record;
mod from_ref;
mod from_string;
mod from_template_literal;
mod from_tuple;
mod from_type;
mod from_union;
#[path = "try/mod.rs"]
mod r#try;

use crate::DiagnosticErrorInfo;
use serde_json::Value;

use super::convert::from_type::from_type;
use crate::arguments::validation::authored::context::Context;
use crate::arguments::validation::authored::schema_view::{Node, View};

/// Convert a supplied value through the authored node it is converted against.
///
/// The returned replacement is what the reference converter returns; a caller that keeps the
/// supplied value only keeps the in-place effects the conversion performed.
pub(crate) fn convert<'a>(
    converter: &mut Context<'a>,
    view: View<'a>,
    value: &mut Value,
) -> Result<Option<Value>, DiagnosticErrorInfo> {
    from_type(converter, &Node::Leaf(view), value)
}

/// Convert one child value and keep its replacement at the child's own slot.
pub(crate) fn convert_member<'a>(
    converter: &mut Context<'a>,
    node: &Node<'a>,
    value: &mut Value,
) -> Result<(), DiagnosticErrorInfo> {
    if let Some(replacement) = from_type(converter, node, value)? {
        *value = replacement;
    }
    Ok(())
}
