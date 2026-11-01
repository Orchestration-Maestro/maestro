//! Enumerated constants spelled as union alternatives.

use std::{borrow::Cow, rc::Rc};

use serde_json::Value;

use crate::arguments::validation::authored::schema_view::Node;

/// Spell an enumerated node's constants as the alternatives conversion selects among.
///
/// The reference keeps string and number constants as literals, the null constant as the
/// null type and every other constant as an alternative that accepts nothing.
pub(crate) fn enum_to_union(constants: &[Value]) -> Vec<Rc<Node<'_>>> {
    constants
        .iter()
        .map(|constant| match constant {
            Value::String(_) | Value::Number(_) => Rc::new(Node::Constant(Cow::Borrowed(constant))),
            Value::Null => Rc::new(Node::Null),
            Value::Bool(_) | Value::Array(_) | Value::Object(_) => Rc::new(Node::Never),
        })
        .collect()
}
