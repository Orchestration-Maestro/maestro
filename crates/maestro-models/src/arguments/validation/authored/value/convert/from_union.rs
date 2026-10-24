//! Conversion through the alternatives of a union node.

use std::rc::Rc;

use serde_json::Value;

use super::from_type::from_type;
use crate::arguments::validation::authored::schema_view::{Node, View};
use crate::arguments::validation::authored::{self, Converter};

/// Convert through the alternatives of an instantiated union node.
///
/// The fast path keeps a value that already matches an alternative; otherwise the first
/// candidate that matches an alternative replaces it, as the reference selects one.
pub(crate) fn from_union<'a>(
    converter: &mut Converter<'a>,
    alternatives: &[Rc<Node<'a>>],
    value: &mut Value,
) -> Option<Value> {
    if matches_any(alternatives, value) {
        return None;
    }
    for alternative in alternatives {
        let mut candidate = value.clone();
        if let Some(replacement) = from_type(converter, alternative, &mut candidate) {
            candidate = replacement;
        }
        if matches_any(alternatives, &candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Convert through the alternatives an authored union declares.
pub(crate) fn from_union_view<'a>(
    converter: &mut Converter<'a>,
    view: View<'a>,
    value: &mut Value,
) -> Option<Value> {
    let alternatives = view.raw.get("anyOf").and_then(Value::as_array)?;
    let nodes: Vec<Rc<Node<'a>>> = alternatives
        .iter()
        .map(|alternative| Rc::new(Node::Leaf(converter.child(view, alternative))))
        .collect();
    from_union(converter, &nodes, value)
}

/// Whether any alternative accepts the supplied value.
fn matches_any(alternatives: &[Rc<Node<'_>>], value: &Value) -> bool {
    alternatives
        .iter()
        .any(|alternative| authored::accepts(alternative, value))
}
