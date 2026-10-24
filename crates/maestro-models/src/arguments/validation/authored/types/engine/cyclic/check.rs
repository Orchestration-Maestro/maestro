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
    let children: Vec<View<'a>> = match view.kind {
        Some(SchemaKind::Ref) => return from_ref(converter, stack, view),
        Some(SchemaKind::Array) => converter.keyword(view, "items").into_iter().collect(),
        Some(SchemaKind::Object) => converter.children(view, "properties").unwrap_or_default(),
        Some(SchemaKind::Tuple) => converter.children(view, "items").unwrap_or_default(),
        Some(SchemaKind::Union) => converter.children(view, "anyOf").unwrap_or_default(),
        Some(SchemaKind::Intersect) => converter.children(view, "allOf").unwrap_or_default(),
        Some(SchemaKind::Record) => view
            .raw
            .get("patternProperties")
            .and_then(Value::as_object)
            .and_then(|patterns| patterns.values().next())
            .map(|value| converter.view(value))
            .into_iter()
            .collect(),
        _ => return false,
    };
    children
        .into_iter()
        .any(|child| cyclic_check(converter, stack, child))
}

/// Whether a reference names a definition already on the stack or one that reaches it.
fn from_ref<'a>(converter: &Converter<'a>, stack: &mut Vec<&'a str>, view: View<'a>) -> bool {
    let Some(name) = view.raw.get("$ref").and_then(Value::as_str) else {
        return false;
    };
    if stack.contains(&name) {
        return true;
    }
    let Some(target) = converter.definition(name) else {
        return false;
    };
    stack.push(name);
    let found = cyclic_check(converter, stack, target);
    stack.pop();
    found
}
