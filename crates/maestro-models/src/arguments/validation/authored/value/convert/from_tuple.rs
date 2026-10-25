//! Conversion through the declared positions of a sequence.

use serde_json::Value;

use super::convert_member;
use crate::arguments::validation::authored::context::Context;
use crate::arguments::validation::authored::schema_view::{Node, View};
use crate::arguments::validation::coercion;

/// Convert the leading positions of a sequence through the nodes an authored tuple declares.
pub(crate) fn from_tuple<'a>(
    converter: &mut Context<'a>,
    view: View<'a>,
    value: &mut Value,
) -> Option<Value> {
    let values = value.as_array_mut()?;
    coercion::positions(view.raw, values, |member, item| {
        let item = Node::Leaf(converter.document.view(item));
        convert_member(converter, &item, member);
    });
    None
}
