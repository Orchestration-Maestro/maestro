//! Distribution of an intersection over its members.

use std::rc::Rc;

use super::{composite, evaluate_intersect, evaluate_type, narrow};
use crate::SchemaKind;
use crate::arguments::validation::authored::schema_view::Node;

/// Distribute the supplied members into the disjoint combinations their intersection keeps.
pub(crate) fn distribute<'a>(types: &[Rc<Node<'a>>]) -> Vec<Rc<Node<'a>>> {
    let mut result: Vec<Rc<Node<'a>>> = Vec::new();
    for left in types {
        result = if left.kind() == Some(SchemaKind::Union) {
            distribute_union(alternatives(left), &result)
        } else {
            distribute_type(left, &result)
        };
    }
    result
}

/// Combine one member with every combination accumulated so far.
fn distribute_type<'a>(node: &Rc<Node<'a>>, types: &[Rc<Node<'a>>]) -> Vec<Rc<Node<'a>>> {
    if types.is_empty() {
        return vec![Rc::clone(node)];
    }
    types
        .iter()
        .map(|left| distribute_operation(node, left))
        .collect()
}

/// Distribute each alternative of a union member into the accumulated combinations.
fn distribute_union<'a>(
    alternatives: &[Rc<Node<'a>>],
    distribution: &[Rc<Node<'a>>],
) -> Vec<Rc<Node<'a>>> {
    let mut result = Vec::new();
    for alternative in alternatives {
        result.extend(distribute_type(alternative, distribution));
    }
    result
}

/// The node one pair of members stands for.
fn distribute_operation<'a>(left: &Rc<Node<'a>>, right: &Rc<Node<'a>>) -> Rc<Node<'a>> {
    let evaluated_left = evaluate_type(left);
    let evaluated_right = evaluate_type(right);
    if is_union(&evaluated_left) || is_union(&evaluated_right) {
        return evaluate_intersect(&[evaluated_left, evaluated_right]);
    }
    let left_object = is_object_like(&evaluated_left);
    let right_object = is_object_like(&evaluated_right);
    match (left_object, right_object) {
        (true, true) => composite::composite(&evaluated_left, &evaluated_right)
            .unwrap_or_else(|| Rc::new(Node::Never)),
        (true, false) => evaluated_left,
        (false, true) => evaluated_right,
        (false, false) => narrow::narrow(&evaluated_left, &evaluated_right),
    }
}

/// Whether a node is an alternative set.
pub(crate) fn is_union(node: &Rc<Node<'_>>) -> bool {
    node.kind() == Some(SchemaKind::Union)
}

/// Whether a node declares named members, as an object or a tuple does.
fn is_object_like(node: &Rc<Node<'_>>) -> bool {
    matches!(node.kind(), Some(SchemaKind::Object | SchemaKind::Tuple))
}

/// The alternatives of a union node.
fn alternatives<'a, 'b>(node: &'b Rc<Node<'a>>) -> &'b [Rc<Node<'a>>] {
    match &**node {
        Node::Union(alternatives, _) => alternatives,
        _ => &[],
    }
}
