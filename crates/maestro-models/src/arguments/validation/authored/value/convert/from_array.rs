//! Conversion through the single declared item kind of a sequence.

use crate::DiagnosticErrorInfo;
use serde_json::Value;

use super::{convert_member, from_type::from_type, r#try::try_array};
use crate::arguments::validation::authored::context::Context;
use crate::arguments::validation::authored::schema_view::{Node, View};
use crate::arguments::validation::coercion;

/// Convert a supplied value through the item node of an authored sequence.
///
/// The returned sequence is a new one, as the reference returns it; nested container
/// mutations reach the supplied sequence's own members.
pub(crate) fn from_array<'a>(
    converter: &mut Context<'a>,
    view: View<'a>,
    value: &mut Value,
) -> Result<Option<Value>, DiagnosticErrorInfo> {
    if !view.raw.get("items").is_some_and(Value::is_object) {
        return Ok(None);
    }
    let Value::Array(source) = value else {
        let mut members = try_array::try_array(value);
        let mut result = Ok(());
        coercion::sequence(view.raw, &mut members, |member, item| {
            let item = Node::Leaf(converter.document.view(item));
            if result.is_ok() {
                result = convert_member(converter, &item, member);
            }
        });
        result?;
        return Ok(Some(Value::Array(members)));
    };
    let mut members = Vec::with_capacity(source.len());
    let mut result = Ok(());
    coercion::sequence(view.raw, source, |member, item| {
        let item = Node::Leaf(converter.document.view(item));
        if result.is_ok() {
            match from_type(converter, &item, member) {
                Ok(replacement) => members.push(replacement.unwrap_or_else(|| member.clone())),
                Err(error) => result = Err(error),
            }
        }
    });
    result?;
    Ok(Some(Value::Array(members)))
}
