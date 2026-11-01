//! Dispatch of the extends check by the left node's kind.

use std::rc::Rc;

use super::{
    any::extends_any, array::extends_array, boolean::extends_boolean, enumeration::extends_enum,
    integer::extends_integer, intersect::extends_intersect, literal::extends_literal,
    never::extends_never, null::extends_null, number::extends_number, object::extends_object,
    string::extends_string, template_literal::extends_template_literal, tuple::extends_tuple,
    union::extends_union, unknown::extends_unknown,
};
use crate::SchemaKind;
use crate::arguments::validation::authored::schema_view::Node;

/// Perform the structural check for the left node's kind.
///
/// A kind the authored JSON contract cannot produce, such as an unresolved reference or a base
/// type, accepts nothing the right node asks for, as it does in the reference.
pub(crate) fn extends_left<'a>(left: &Rc<Node<'a>>, right: &Rc<Node<'a>>) -> bool {
    match left.kind() {
        Some(SchemaKind::Any) => extends_any(),
        Some(SchemaKind::Array) => extends_array(left, right),
        Some(SchemaKind::Boolean) => extends_boolean(left, right),
        Some(SchemaKind::Enum) => extends_enum(left, right),
        Some(SchemaKind::Integer) => extends_integer(left, right),
        Some(SchemaKind::Intersect) => extends_intersect(left, right),
        Some(SchemaKind::Literal) => extends_literal(left, right),
        Some(SchemaKind::Never) => extends_never(),
        Some(SchemaKind::Null) => extends_null(left, right),
        Some(SchemaKind::Number) => extends_number(left, right),
        Some(SchemaKind::Object) => extends_object(left, right),
        Some(SchemaKind::String) => extends_string(left, right),
        Some(SchemaKind::TemplateLiteral) => extends_template_literal(left, right),
        Some(SchemaKind::Tuple) => extends_tuple(left, right),
        Some(SchemaKind::Union) => extends_union(left, right),
        Some(SchemaKind::Unknown) => extends_unknown(right),
        _ => false,
    }
}
