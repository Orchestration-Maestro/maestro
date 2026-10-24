//! Substitution of one reference by the definition it names.

use std::rc::Rc;

use super::super::instantiate::instantiate_ref;
use crate::arguments::validation::authored::schema_view::{Converter, Node, View};

/// Substitute a reference whose definition is visible, keeping a cyclic one in place.
pub(crate) fn ref_instantiate<'a>(
    converter: &Converter<'a>,
    view: View<'a>,
    _stack: &mut Vec<&'a str>,
) -> Rc<Node<'a>> {
    let Some(name) = view.raw.get("$ref").and_then(serde_json::Value::as_str) else {
        return Rc::new(Node::Leaf(view));
    };
    if converter.definition(view, name).is_none() {
        return Rc::new(Node::Leaf(view));
    }
    instantiate_ref(converter, view, name)
}
