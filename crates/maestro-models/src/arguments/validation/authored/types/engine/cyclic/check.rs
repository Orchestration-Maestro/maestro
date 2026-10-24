//! Classification of definitions that recurse through themselves.

use serde_json::Value;

use crate::SchemaKind;
use crate::arguments::validation::authored::schema_view::{Converter, View};

/// Whether the supplied node reaches one of the names already on the stack.
///
/// The reference classifies a reference by walking its own definition once, so a cycle is a
/// name that reappears on that walk. Keeping such a reference is what lets a recursive schema
/// convert without expanding forever.
pub(crate) fn cyclic_check<'a>(
    converter: &Converter<'a>,
    stack: &mut Vec<&'a str>,
    view: View<'a>,
) -> bool {
    match view.kind {
        Some(SchemaKind::Ref) => {
            let Some(name) = view.raw.get("$ref").and_then(Value::as_str) else {
                return false;
            };
            if stack.contains(&name) {
                return true;
            }
            let Some(target) = converter.definition(view, name) else {
                return false;
            };
            stack.push(name);
            let found = cyclic_check(converter, stack, target);
            stack.pop();
            found
        }
        Some(SchemaKind::Array) => converter
            .keyword(view, "items")
            .is_some_and(|item| cyclic_check(converter, stack, item)),
        Some(SchemaKind::Object) => members(converter, view, "properties").is_some_and(|members| {
            members
                .iter()
                .any(|member| cyclic_check(converter, stack, *member))
        }),
        Some(SchemaKind::Tuple) => members(converter, view, "items").is_some_and(|members| {
            members
                .iter()
                .any(|member| cyclic_check(converter, stack, *member))
        }),
        Some(SchemaKind::Union) => members(converter, view, "anyOf").is_some_and(|members| {
            members
                .iter()
                .any(|member| cyclic_check(converter, stack, *member))
        }),
        Some(SchemaKind::Intersect) => members(converter, view, "allOf").is_some_and(|members| {
            members
                .iter()
                .any(|member| cyclic_check(converter, stack, *member))
        }),
        Some(SchemaKind::Record) => {
            record_value(converter, view).is_some_and(|value| cyclic_check(converter, stack, value))
        }
        _ => false,
    }
}

/// The child nodes a keyword names, in declaration order.
fn members<'a>(converter: &Converter<'a>, view: View<'a>, keyword: &str) -> Option<Vec<View<'a>>> {
    let declared = view.raw.get(keyword)?;
    let children: Vec<&'a Value> = match declared {
        Value::Object(members) => members.values().collect(),
        Value::Array(members) => members.iter().collect(),
        _ => return None,
    };
    Some(
        children
            .into_iter()
            .map(|child| converter.child(view, child))
            .collect(),
    )
}

/// The value node a record's declared pattern names.
fn record_value<'a>(converter: &Converter<'a>, view: View<'a>) -> Option<View<'a>> {
    let patterns = view
        .raw
        .get("patternProperties")
        .and_then(Value::as_object)?;
    let value = patterns.values().next()?;
    Some(converter.child(view, value))
}
