//! Tuple positions spelled as named object properties.
use crate::arguments::validation::authored::schema_view::{Member, Node, ObjectNode};
use crate::arguments::validation::authored::types::extends::tuple::tuple_items;
use std::{borrow::Cow, rc::Rc};
/// The object node whose properties are the supplied tuple's positions.
pub(crate) fn to_object<'a>(node: &Rc<Node<'a>>) -> Option<Rc<Node<'a>>> {
    let items = tuple_items(node)?;
    Some(Rc::new(Node::Object(ObjectNode {
        source: None,
        properties: items
            .iter()
            .enumerate()
            .map(|(index, item)| Member {
                name: Cow::Owned(index.to_string()),
                node: Rc::clone(item),
                optional: false,
            })
            .collect(),
    })))
}
