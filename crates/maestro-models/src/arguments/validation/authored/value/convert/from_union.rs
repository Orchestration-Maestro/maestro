//! Conversion through the alternatives of a union node.

use std::rc::Rc;

use crate::DiagnosticErrorInfo;
use serde_json::Value;

use super::from_type::from_type;
use crate::arguments::validation::authored::schema_view::{Node, View};
use crate::arguments::validation::authored::{self, context::Context};

/// Convert through the alternatives of a union node.
///
/// The fast path keeps a value that already matches an alternative; otherwise the first
/// candidate the whole union accepts replaces it, as the reference selects one. The whole
/// union is its authored node when there is one, with that node's options.
pub(crate) fn from_union<'a>(
    converter: &mut Context<'a>,
    alternatives: &[Rc<Node<'a>>],
    source: Option<View<'a>>,
    value: &mut Value,
) -> Result<Option<Value>, DiagnosticErrorInfo> {
    if matches_any(converter, alternatives, value)? {
        return Ok(None);
    }
    let mut candidates = Vec::with_capacity(alternatives.len());
    for alternative in alternatives {
        let mut candidate = value.clone();
        if let Some(replacement) = from_type(converter, alternative, &mut candidate)? {
            candidate = replacement;
        }
        candidates.push(candidate);
    }
    for mut candidate in candidates {
        let selected = match source {
            Some(view) => authored::accepts(converter, &Node::Leaf(view), &mut candidate)?,
            None => matches_any(converter, alternatives, &mut candidate)?,
        };
        if selected {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

/// Convert through the alternatives an authored union declares.
pub(crate) fn from_union_view<'a>(
    converter: &mut Context<'a>,
    view: View<'a>,
    value: &mut Value,
) -> Result<Option<Value>, DiagnosticErrorInfo> {
    let Some(children) = view.children("anyOf") else {
        return Ok(None);
    };
    let alternatives: Vec<Rc<Node<'a>>> = children
        .into_iter()
        .map(|alternative| Rc::new(Node::Leaf(alternative)))
        .collect();
    from_union(converter, &alternatives, Some(view), value)
}

/// Whether any alternative accepts the supplied value.
fn matches_any(
    converter: &Context<'_>,
    alternatives: &[Rc<Node<'_>>],
    value: &mut Value,
) -> Result<bool, DiagnosticErrorInfo> {
    for alternative in alternatives {
        if authored::accepts(converter, alternative, value)? {
            return Ok(true);
        }
    }
    Ok(false)
}
