//! Conversion through the instantiated and evaluated members of an intersection.

use std::rc::Rc;

use serde_json::Value;

use super::from_type::from_type;
use crate::arguments::validation::authored::schema_view::{Converter, Node, View};
use crate::arguments::validation::authored::types::engine::evaluate::{
    evaluate_intersect, evaluate_type,
};
use crate::arguments::validation::authored::types::engine::instantiate::instantiate;

/// Convert through the evaluated intersection of an instantiated node.
pub(crate) fn from_intersect<'a>(
    converter: &mut Converter<'a>,
    members: &[Rc<Node<'a>>],
    value: &mut Value,
) -> Option<Value> {
    let evaluated = evaluate_intersect(converter, members);
    from_type(converter, &evaluate_type(converter, &evaluated), value)
}

/// Instantiate, evaluate and convert the members an authored intersection declares.
pub(crate) fn from_intersect_view<'a>(
    converter: &mut Converter<'a>,
    view: View<'a>,
    value: &mut Value,
) -> Option<Value> {
    let members = view.raw.get("allOf").and_then(Value::as_array)?;
    let members: Vec<Rc<Node<'a>>> = members
        .iter()
        .map(|member| instantiate(converter, converter.child(view, member)))
        .collect();
    from_intersect(converter, &members, value)
}
