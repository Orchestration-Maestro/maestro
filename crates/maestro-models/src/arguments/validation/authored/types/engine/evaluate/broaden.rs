//! Broadening of distributed members into the most general node they share.

use std::rc::Rc;

use super::{
    compare::{Relation, compare},
    evaluate_type,
    flatten::flatten,
};
use crate::SchemaKind;
use crate::arguments::validation::authored::schema_view::Node;

/// Broaden supplied members into one node, an alternative set or an impossible node.
pub(crate) fn broaden<'a>(types: &[Rc<Node<'a>>]) -> Rc<Node<'a>> {
    let broadened = broaden_types(types);
    let flattened = flatten(&broadened);
    match flattened.len() {
        0 => Rc::new(Node::Never),
        1 => Rc::clone(&flattened[0]),
        _ => Rc::new(Node::Union(flattened, None)),
    }
}

/// Fold the supplied members, dropping the ones a later member already covers.
fn broaden_types<'a>(types: &[Rc<Node<'a>>]) -> Vec<Rc<Node<'a>>> {
    let mut result: Vec<Rc<Node<'a>>> = Vec::new();
    for node in types {
        match node.kind() {
            Some(SchemaKind::Object) => result.push(Rc::clone(node)),
            Some(SchemaKind::Never) => {}
            _ => result = broaden_type(node, result),
        }
    }
    result
}

/// Broaden one member into the accumulated set.
fn broaden_type<'a>(node: &Rc<Node<'a>>, types: Vec<Rc<Node<'a>>>) -> Vec<Rc<Node<'a>>> {
    let evaluated = evaluate_type(node);
    if evaluated.kind() == Some(SchemaKind::Any) {
        return vec![evaluated];
    }
    if !is_broadest(&evaluated, &types) {
        return types;
    }
    let mut filtered: Vec<Rc<Node<'a>>> = types
        .into_iter()
        .filter(|left| compare(&evaluated, left) != Relation::RightInside)
        .collect();
    filtered.push(evaluated);
    filtered
}

/// Whether no supplied member already covers the supplied node.
fn is_broadest<'a>(node: &Rc<Node<'a>>, types: &[Rc<Node<'a>>]) -> bool {
    !types
        .iter()
        .any(|left| matches!(compare(node, left), Relation::LeftInside | Relation::Equal))
}
