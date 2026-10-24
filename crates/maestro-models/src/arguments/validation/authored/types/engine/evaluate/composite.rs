//! Composition of two object-like members into one object.

use std::{borrow::Cow, rc::Rc};

use super::evaluate_intersect;
use crate::arguments::validation::authored::schema_view::{Converter, Node, ObjectNode};

/// Compose the members both object-like nodes declare.
///
/// Members keep the reference's composition order: the right operand's names first, then the
/// names only the left operand declares. A name both operands declare is narrowed to their
/// intersection; a name neither declares belongs to neither.
pub(crate) fn composite<'a>(
    converter: &Converter<'a>,
    left: &Rc<Node<'a>>,
    right: &Rc<Node<'a>>,
) -> Option<Rc<Node<'a>>> {
    let left_members = left.members()?;
    let right_members = right.members()?;
    let mut names: Vec<Cow<'a, str>> = right_members.iter().map(|(name, _)| name.clone()).collect();
    for (name, _) in &left_members {
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
    let mut properties = Vec::with_capacity(names.len());
    for name in names {
        let member = match (find(&left_members, &name), find(&right_members, &name)) {
            (Some(left), Some(right)) => {
                evaluate_intersect(converter, &[Rc::clone(left), Rc::clone(right)])
            }
            (Some(left), None) => Rc::clone(left),
            (None, Some(right)) => Rc::clone(right),
            (None, None) => Rc::new(Node::Never),
        };
        properties.push((name, member));
    }
    Some(Rc::new(Node::Object(ObjectNode {
        properties,
        source: None,
    })))
}

/// The member the supplied name declares.
fn find<'a, 'b>(
    members: &'b [(Cow<'a, str>, Rc<Node<'a>>)],
    name: &str,
) -> Option<&'b Rc<Node<'a>>> {
    members
        .iter()
        .find(|(member, _)| member.as_ref() == name)
        .map(|(_, node)| node)
}
