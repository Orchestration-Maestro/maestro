//! Structural comparison of schema nodes.

use std::rc::Rc;

use self::extends_left::extends_left;
use crate::SchemaKind;
use crate::arguments::validation::authored::schema_view::Node;
use crate::arguments::validation::authored::types::engine::cyclic::extends::cyclic_extends;

pub(crate) mod any;
pub(crate) mod array;
pub(crate) mod boolean;
pub(crate) mod enumeration;
pub(crate) mod extends_left;
pub(crate) mod extends_right;
pub(crate) mod integer;
pub(crate) mod intersect;
pub(crate) mod literal;
pub(crate) mod never;
pub(crate) mod null;
pub(crate) mod number;
pub(crate) mod object;
pub(crate) mod string;
pub(crate) mod template_literal;
pub(crate) mod tuple;
pub(crate) mod union;
pub(crate) mod unknown;

/// Whether every value the left node accepts is accepted by the right node.
pub(crate) fn extends<'a>(left: &Rc<Node<'a>>, right: &Rc<Node<'a>>) -> bool {
    let left = canonical(left);
    let right = canonical(right);
    extends_left(&left, &right)
}

/// The node the check compares, spelling out a cyclic node's definition.
fn canonical<'a>(node: &Rc<Node<'a>>) -> Rc<Node<'a>> {
    match &**node {
        Node::Leaf(view) if view.kind == Some(&SchemaKind::Cyclic) => cyclic_extends(*view),
        _ => Rc::clone(node),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::rc::Rc;

    use serde_json::{Value, json};

    use super::extends;
    use crate::SchemaKind;
    use crate::arguments::validation::authored::context::Context;
    use crate::arguments::validation::authored::schema_view::{Document, Node};
    use crate::arguments::validation::authored::types::engine::instantiate::instantiate;

    /// Compare two authored operands the way conversion asks the question.
    #[track_caller]
    fn compare(operands: [Value; 2], kinds: &[(&str, SchemaKind)]) -> bool {
        let schema = Value::Array(operands.into_iter().collect());
        let kinds: HashMap<String, SchemaKind> = kinds
            .iter()
            .map(|(pointer, kind)| ((*pointer).to_owned(), kind.clone()))
            .collect();
        let document = Document::new(&schema, &kinds);
        let code = crate::arguments::validation::check::Code::default();
        let converter = Context::new(&document, &code);
        let left: Rc<Node<'_>> = instantiate(&converter, converter.document.view(&schema[0]));
        let right: Rc<Node<'_>> = instantiate(&converter, converter.document.view(&schema[1]));
        extends(&left, &right)
    }

    /// An alternative set extends what every one of its alternatives extends.
    #[test]
    fn alternatives_do_not_extend_a_sequence_of_the_same_positions() {
        assert!(!compare(
            [
                json!({"anyOf":[{"type":"string"},{"type":"number"}]}),
                json!({"type":"array","items":[{"type":"string"},{"type":"number"}]}),
            ],
            &[
                ("/0", SchemaKind::Union),
                ("/0/anyOf/0", SchemaKind::String),
                ("/0/anyOf/1", SchemaKind::Number),
                ("/1", SchemaKind::Tuple),
                ("/1/items/0", SchemaKind::String),
                ("/1/items/1", SchemaKind::Number),
            ],
        ));
    }

    /// An alternative set never becomes an intersection of its alternatives.
    #[test]
    fn alternatives_do_not_extend_an_intersection_of_the_same_members() {
        assert!(!compare(
            [
                json!({"anyOf":[{"type":"string"},{"type":"number"}]}),
                json!({"allOf":[{"type":"string"},{"type":"number"}]}),
            ],
            &[
                ("/0", SchemaKind::Union),
                ("/0/anyOf/0", SchemaKind::String),
                ("/0/anyOf/1", SchemaKind::Number),
                ("/1", SchemaKind::Intersect),
                ("/1/allOf/0", SchemaKind::String),
                ("/1/allOf/1", SchemaKind::Number),
            ],
        ));
    }

    /// A member an object leaves optional cannot satisfy a required member beside it.
    #[test]
    fn an_optional_member_never_satisfies_a_required_member() {
        assert!(!compare(
            [
                json!({"type":"object","required":["a"],"properties":{"a":{"type":"string"},"b":{"type":"string"}}}),
                json!({"type":"object","required":["a","b"],"properties":{"a":{"type":"string"},"b":{"type":"string"}}}),
            ],
            &[
                ("/0", SchemaKind::Object),
                ("/1", SchemaKind::Object),
                ("/0/properties/a", SchemaKind::String),
                ("/0/properties/b", SchemaKind::String),
                ("/1/properties/a", SchemaKind::String),
                ("/1/properties/b", SchemaKind::String),
            ],
        ));
    }

    /// A member the right object leaves out of `required` is extended by its absence.
    #[test]
    fn an_absent_member_still_extends_an_optional_member() {
        assert!(compare(
            [
                json!({"type":"object","required":["a"],"properties":{"a":{"type":"string"}}}),
                json!({"type":"object","required":["a"],"properties":{"a":{"type":"string"},"b":{"type":"string"}}}),
            ],
            &[
                ("/0", SchemaKind::Object),
                ("/1", SchemaKind::Object),
                ("/0/properties/a", SchemaKind::String),
                ("/1/properties/a", SchemaKind::String),
                ("/1/properties/b", SchemaKind::String),
            ],
        ));
    }
}
