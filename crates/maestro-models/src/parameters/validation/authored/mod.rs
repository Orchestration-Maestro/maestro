//! Authored schema conversion applied before the unchanged plain pass.
//!
//! A schema node the author built carries its kind; a node present only as JSON does not.
//! Conversion visits the tagged nodes, so an untagged node keeps its supplied value and the
//! later plain pass observes it as ordinary JSON Schema.
mod context;
mod schema_view;
mod types;
mod value;

use std::collections::HashMap;

use serde_json::Value;

use crate::SchemaKind;
use context::Context;
use schema_view::{Document, Node};

/// Convert one argument candidate through the kinds its schema author declared.
///
/// Only in-place effects are kept, as the reference keeps them: a replacement a converter
/// returns for the root is discarded, while container mutations reach the supplied value.
pub(super) fn convert(value: &mut Value, schema: &Value, kinds: &HashMap<String, SchemaKind>) {
    let document = Document::new(schema, kinds);
    let mut converter = Context::new(&document);
    let root = converter.document.root();
    let _replacement = value::convert::convert(&mut converter, root, value);
}

/// Whether the supplied node accepts the supplied value under the current definitions.
///
/// An authored node, and a composed node that keeps the authored node it was instantiated
/// from, is checked as that node with its options and modifiers, its references reading the
/// current definitions first. A node composition built has only the structure it was given.
pub(super) fn accepts(converter: &Context<'_>, node: &Node<'_>, value: &Value) -> bool {
    let source = match node {
        Node::Leaf(view) => Some(*view),
        Node::Object(object) => object.source,
        Node::Array(_, source)
        | Node::Tuple(_, source)
        | Node::Union(_, source)
        | Node::Intersect(_, source) => *source,
        _ => None,
    };
    if let Some(view) = source {
        return super::check::check(view.raw, converter.frames(), value)
            .is_ok_and(|errors| errors.is_empty());
    }
    match node {
        Node::Constant(constant) => super::scalars::equal(constant, value),
        Node::Null => value.is_null(),
        Node::Any | Node::Unknown => true,
        Node::Object(object) => value.as_object().is_some_and(|members| {
            object.properties.iter().all(|member| {
                members
                    .get(member.name.as_ref())
                    .map_or(member.optional, |value| {
                        accepts(converter, &member.node, value)
                    })
            })
        }),
        Node::Array(item, _) => value
            .as_array()
            .is_some_and(|values| values.iter().all(|value| accepts(converter, item, value))),
        Node::Tuple(items, _) => value.as_array().is_some_and(|values| {
            values.len() == items.len()
                && values
                    .iter()
                    .zip(items)
                    .all(|(value, item)| accepts(converter, item, value))
        }),
        Node::Union(alternatives, _) => alternatives
            .iter()
            .any(|alternative| accepts(converter, alternative, value)),
        Node::Intersect(members, _) => members
            .iter()
            .all(|member| accepts(converter, member, value)),
        Node::Leaf(_) | Node::Never => false,
    }
}
