//! Composition of two object-like members into one object.

use std::rc::Rc;

use super::evaluate_intersect;
use crate::arguments::validation::authored::schema_view::{Member, Members, Node, ObjectNode};

/// Compose the members both object-like nodes declare.
///
/// Members keep the reference's composition order: the right operand's names first, then the
/// names only the left operand declares. A name both operands declare is narrowed to their
/// intersection and stays optional only when both operands leave it optional.
pub(crate) fn composite<'a>(left: &Rc<Node<'a>>, right: &Rc<Node<'a>>) -> Option<Rc<Node<'a>>> {
    let left = if left.kind() == Some(crate::SchemaKind::Tuple) {
        crate::arguments::validation::authored::types::engine::tuple::to_object::to_object(left)?
    } else {
        Rc::clone(left)
    };
    let right = if right.kind() == Some(crate::SchemaKind::Tuple) {
        crate::arguments::validation::authored::types::engine::tuple::to_object::to_object(right)?
    } else {
        Rc::clone(right)
    };
    let left_members = crate::arguments::validation::authored::types::object::properties(&left)?;
    let right_members = crate::arguments::validation::authored::types::object::properties(&right)?;
    let left_index = Members::new(&left_members);
    let right_index = Members::new(&right_members);
    let mut properties: Vec<Member<'a>> = right_members
        .iter()
        .map(|right| match left_index.get(&right.name) {
            Some(left) => Member {
                name: right.name.clone(),
                node: evaluate_intersect(&[Rc::clone(&left.node), Rc::clone(&right.node)]),
                optional: left.optional && right.optional,
            },
            None => right.clone(),
        })
        .collect();
    properties.extend(
        left_members
            .iter()
            .filter(|left| right_index.get(&left.name).is_none())
            .cloned(),
    );
    Some(Rc::new(Node::Object(ObjectNode {
        properties,
        source: None,
    })))
}

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::HashMap;

    use serde_json::{Map, Value};

    use super::composite;
    use crate::SchemaKind;
    use crate::arguments::validation::authored::context::Context;
    use crate::arguments::validation::authored::schema_view::MEMBER_LOOKUPS;
    use crate::arguments::validation::authored::types::engine::instantiate::instantiate;

    /// Two objects of the supplied width with disjoint member names, and their kinds.
    pub(crate) fn operands(width: usize) -> (Value, HashMap<String, SchemaKind>) {
        let mut kinds = HashMap::new();
        let operands: Vec<Value> = ["left", "right"]
            .into_iter()
            .enumerate()
            .map(|(operand, side)| {
                kinds.insert(format!("/{operand}"), SchemaKind::Object);
                let properties: Map<String, Value> = (0..width)
                    .map(|index| {
                        kinds.insert(
                            format!("/{operand}/properties/{side}{index}"),
                            SchemaKind::String,
                        );
                        (
                            format!("{side}{index}"),
                            serde_json::json!({"type": "string"}),
                        )
                    })
                    .collect();
                serde_json::json!({"type": "object", "properties": properties})
            })
            .collect();
        (Value::Array(operands), kinds)
    }

    /// Composing two objects looks each member name up a bounded number of times.
    #[test]
    fn composition_looks_each_member_up_once_per_operand() {
        for width in [16, 32, 64] {
            let (schema, kinds) = operands(width);
            let document =
                crate::arguments::validation::authored::schema_view::Document::new(&schema, &kinds);
            let code = crate::arguments::validation::check::Code::default();
            let converter = Context::new(&document, &code);
            let left = instantiate(&converter, converter.document.view(&schema[0]));
            let right = instantiate(&converter, converter.document.view(&schema[1]));
            MEMBER_LOOKUPS.with(|count| count.set(0));
            let composed = composite(&left, &right);
            let lookups = MEMBER_LOOKUPS.with(std::cell::Cell::get);
            assert_eq!(
                composed
                    .as_deref()
                    .and_then(crate::arguments::validation::authored::types::object::properties)
                    .map(|members| members.len()),
                Some(2 * width)
            );
            assert!(
                lookups <= 2 * width,
                "{lookups} lookups for {width} members per operand"
            );
        }
    }
}
