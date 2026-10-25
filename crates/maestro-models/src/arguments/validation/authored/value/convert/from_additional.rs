//! Conversion of the members no declared entry covers.

use crate::DiagnosticErrorInfo;
use serde_json::Map;

use super::from_type::from_type;
use crate::arguments::validation::authored::context::Context;
use crate::arguments::validation::authored::schema_view::Node;

/// Convert every member no declared entry matches through the additional node.
///
/// The reference applies its additional schema once per declared entry, which converts a
/// member as often as the entry count; Maestro applies it once to the uncovered complement,
/// so a member keeps the single conversion its own node already gave it.
pub(crate) fn from_additional<'a>(
    converter: &mut Context<'a>,
    object: &mut Map<String, serde_json::Value>,
    entries: &[(&str, regress::Regex)],
    additional: &Node<'a>,
) -> Result<(), DiagnosticErrorInfo> {
    for (name, member) in object.iter_mut() {
        if entries
            .iter()
            .any(|(_, expression)| expression.find(name).is_some())
        {
            continue;
        }
        if let Some(replacement) = from_type(converter, additional, member)? {
            *member = replacement;
        }
    }
    Ok(())
}
