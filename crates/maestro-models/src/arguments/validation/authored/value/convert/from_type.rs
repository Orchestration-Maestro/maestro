//! Dispatch of a value through the node it is converted against.

use serde_json::Value;

use super::{
    from_array, from_boolean, from_cyclic, from_enum, from_integer, from_intersect, from_literal,
    from_null, from_number, from_object, from_record, from_ref, from_string, from_template_literal,
    from_tuple, from_union,
};
use crate::SchemaKind;
use crate::arguments::validation::authored::context::Context;
use crate::arguments::validation::authored::schema_view::{Node, View};

/// Convert a supplied value through one schema node.
///
/// `Some` is the replacement the reference converter returns for this node; `None` means the
/// supplied value is already the conversion result, after any in-place effects. A sequence
/// converts through the authored node it was instantiated from; only structural comparison
/// builds a sequence without one, and conversion never meets it.
pub(crate) fn from_type<'a>(
    converter: &mut Context<'a>,
    node: &Node<'a>,
    value: &mut Value,
) -> Option<Value> {
    match node {
        Node::Leaf(view) => from_leaf(converter, *view, value),
        Node::Constant(constant) => from_literal::from_literal(constant, value),
        Node::Null => from_null::from_null(value),
        Node::Object(object) => from_object::from_object(converter, object, value),
        Node::Array(_, source) => {
            source.and_then(|view| from_array::from_array(converter, view, value))
        }
        Node::Tuple(_, source) => {
            source.and_then(|view| from_tuple::from_tuple(converter, view, value))
        }
        Node::Union(alternatives, source) => {
            from_union::from_union(converter, alternatives, *source, value)
        }
        Node::Intersect(members, _) => from_intersect::from_intersect(converter, members, value),
        Node::Never | Node::Any | Node::Unknown => None,
    }
}

/// Convert through the kind an authored node declares.
fn from_leaf<'a>(converter: &mut Context<'a>, view: View<'a>, value: &mut Value) -> Option<Value> {
    match view.kind {
        Some(SchemaKind::String) => from_string::from_string(value),
        Some(SchemaKind::Number) => from_number::from_number(value),
        Some(SchemaKind::Integer) => from_integer::from_integer(value),
        Some(SchemaKind::Boolean) => from_boolean::from_boolean(value),
        Some(SchemaKind::Null) => from_null::from_null(value),
        Some(SchemaKind::Array) => from_array::from_array(converter, view, value),
        Some(SchemaKind::Tuple) => from_tuple::from_tuple(converter, view, value),
        Some(SchemaKind::Object) => from_object::from_object_view(converter, view, value),
        Some(SchemaKind::Record) => from_record::from_record(converter, view, value),
        Some(SchemaKind::Literal) => from_literal::from_literal_view(view, value),
        Some(SchemaKind::Enum) => from_enum::from_enum(converter, view, value),
        Some(SchemaKind::Union) => from_union::from_union_view(converter, view, value),
        Some(SchemaKind::Intersect) => from_intersect::from_intersect_view(converter, view, value),
        Some(SchemaKind::Ref) => from_ref::from_ref(converter, view, value),
        Some(SchemaKind::Cyclic) => from_cyclic::from_cyclic(converter, view, value),
        Some(SchemaKind::TemplateLiteral) => {
            from_template_literal::from_template_literal(converter, view, value)
        }
        _ => None,
    }
}
