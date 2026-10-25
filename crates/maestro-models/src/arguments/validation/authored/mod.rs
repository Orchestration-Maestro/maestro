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

use crate::DiagnosticErrorInfo;
use serde_json::Value;

use crate::SchemaKind;
use context::Context;
pub(super) use schema_view::index;
use schema_view::{Document, Node};

/// Convert one argument candidate through the kinds its schema author declared.
///
/// Only in-place effects are kept, as the reference keeps them: a replacement a converter
/// returns for the root is discarded, while container mutations reach the supplied value.
pub(super) fn convert(
    value: &mut Value,
    schema: &Value,
    kinds: &HashMap<String, SchemaKind>,
    code: &super::check::Code<'_>,
) -> Result<(), DiagnosticErrorInfo> {
    let document = Document::new(schema, kinds);
    let mut converter = Context::new(&document, code);
    let root = converter.document.root();
    value::convert::convert(&mut converter, root, value)?;
    Ok(())
}

/// Whether the supplied node accepts the supplied value under the current definitions.
///
/// An authored node, and a composed node that keeps the authored node it was instantiated
/// from, is checked as that node with its options and modifiers, its references reading the
/// current definitions first. A node composition built has only the structure it was given.
pub(super) fn accepts(
    converter: &Context<'_>,
    node: &Node<'_>,
    value: &mut Value,
) -> Result<bool, DiagnosticErrorInfo> {
    if let Some(view) = node.source() {
        return super::check::authored_candidate(
            view.raw,
            converter.frames(),
            value,
            converter.code,
        );
    }
    Ok(match node {
        Node::Constant(constant) => super::scalars::equal(constant, value),
        Node::Null => value.is_null(),
        Node::Any | Node::Unknown => true,
        Node::Object(object) => return accepts_object(converter, object, value),
        Node::Array(item, _) => {
            let Some(values) = value.as_array_mut() else {
                return Ok(false);
            };
            for value in values {
                if !accepts(converter, item, value)? {
                    return Ok(false);
                }
            }
            true
        }
        Node::Tuple(items, _) => {
            let Some(values) = value
                .as_array_mut()
                .filter(|values| values.len() == items.len())
            else {
                return Ok(false);
            };
            for (value, item) in values.iter_mut().zip(items) {
                if !accepts(converter, item, value)? {
                    return Ok(false);
                }
            }
            true
        }
        Node::Union(alternatives, _) => {
            for alternative in alternatives {
                if accepts(converter, alternative, value)? {
                    return Ok(true);
                }
            }
            false
        }
        Node::Intersect(members, _) => {
            for member in members {
                if !accepts(converter, member, value)? {
                    return Ok(false);
                }
            }
            true
        }
        Node::Leaf(_) | Node::Never => false,
    })
}

/// Check composed object members in declaration order with fallible child acceptance.
fn accepts_object(
    converter: &Context<'_>,
    object: &schema_view::ObjectNode<'_>,
    value: &mut Value,
) -> Result<bool, DiagnosticErrorInfo> {
    let Some(members) = value.as_object_mut() else {
        return Ok(false);
    };
    for member in &object.properties {
        let accepted = match members.get_mut(member.name.as_ref()) {
            Some(value) => accepts(converter, &member.node, value)?,
            None => member.optional,
        };
        if !accepted {
            return Ok(false);
        }
    }
    Ok(true)
}
