//! Conversion through the instantiated and evaluated members of an intersection.

use std::rc::Rc;

use serde_json::Value;

use super::from_type::from_type;
use crate::arguments::validation::authored::context::Context;
use crate::arguments::validation::authored::schema_view::{Node, View};
use crate::arguments::validation::authored::types::engine::evaluate::{
    evaluate_intersect, evaluate_type,
};
use crate::arguments::validation::authored::types::engine::instantiate::instantiate;

/// Convert through the evaluated intersection of an instantiated node.
pub(crate) fn from_intersect<'a>(
    converter: &mut Context<'a>,
    members: &[Rc<Node<'a>>],
    value: &mut Value,
) -> Option<Value> {
    let evaluated = evaluate_intersect(members);
    from_type(converter, &evaluate_type(&evaluated), value)
}

/// Instantiate, evaluate and convert the members an authored intersection declares.
pub(crate) fn from_intersect_view<'a>(
    converter: &mut Context<'a>,
    view: View<'a>,
    value: &mut Value,
) -> Option<Value> {
    let members: Vec<Rc<Node<'a>>> = view
        .children("allOf")?
        .into_iter()
        .map(|member| instantiate(converter, member))
        .collect();
    from_intersect(converter, &members, value)
}
