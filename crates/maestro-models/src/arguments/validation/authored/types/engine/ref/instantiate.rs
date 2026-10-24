//! Substitution of one reference by the definition it names.

use std::rc::Rc;

use super::super::{cyclic::check::cyclic_check, instantiate::instantiate};
use crate::arguments::validation::authored::context::Context;
use crate::arguments::validation::authored::schema_view::{Node, View};

/// Substitute a reference by its visible definition, keeping one that recurses in place.
pub(crate) fn ref_instantiate<'a>(converter: &Context<'a>, view: View<'a>) -> Rc<Node<'a>> {
    let target = view
        .raw
        .get("$ref")
        .and_then(serde_json::Value::as_str)
        .and_then(|name| Some((name, converter.definition(name)?)));
    match target {
        Some((name, target)) if !cyclic_check(converter, &mut vec![name], target) => {
            instantiate(converter, target)
        }
        _ => Rc::new(Node::Leaf(view)),
    }
}
