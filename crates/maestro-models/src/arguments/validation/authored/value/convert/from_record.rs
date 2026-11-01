//! Conversion through the pattern members of a record.

use crate::DiagnosticErrorInfo;
use serde_json::Value;

use super::from_additional::from_additional;
use super::from_type::from_type;
use crate::arguments::validation::authored::context::Context;
use crate::arguments::validation::authored::schema_view::{Node, View};
use crate::arguments::validation::authored::types::engine::patterns::pattern::pattern;

/// Convert the members each declared pattern covers, then the rest.
pub(crate) fn from_record<'a>(
    converter: &mut Context<'a>,
    view: View<'a>,
    value: &mut Value,
) -> Result<Option<Value>, DiagnosticErrorInfo> {
    let Some(object) = value.as_object_mut() else {
        return Ok(None);
    };
    let Some(patterns) = view.raw.get("patternProperties").and_then(Value::as_object) else {
        return Ok(None);
    };
    let entries: Vec<(&str, regress::Regex)> = patterns
        .iter()
        .filter_map(|(source, _)| Some((source.as_str(), pattern(source)?)))
        .collect();
    for (source, expression) in &entries {
        let Some(child) = patterns.get(*source) else {
            continue;
        };
        let child = Node::Leaf(converter.document.view(child));
        for (name, member) in object.iter_mut() {
            if expression.find(name).is_some()
                && let Some(replacement) = from_type(converter, &child, member)?
            {
                *member = replacement;
            }
        }
    }
    if let Some(additional) = view
        .raw
        .get("additionalProperties")
        .filter(|schema| schema.is_object())
    {
        let additional = Node::Leaf(converter.document.view(additional));
        from_additional(converter, object, &entries, &additional)?;
    }
    Ok(None)
}
