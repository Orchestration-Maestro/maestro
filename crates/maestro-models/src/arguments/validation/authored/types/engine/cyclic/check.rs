//! Classification of definitions that recurse through themselves.

use serde_json::Value;

use crate::SchemaKind;
use crate::arguments::validation::authored::context::Context;
use crate::arguments::validation::authored::schema_view::View;

/// Whether the supplied node reaches one of the names already on the stack.
///
/// The reference classifies a reference by walking its own definition once, so a cycle is a
/// name that reappears on that walk. Keeping such a reference is what lets a recursive schema
/// convert without expanding forever.
pub(crate) fn cyclic_check<'a>(
    converter: &Context<'a>,
    stack: &mut Vec<&'a str>,
    view: View<'a>,
) -> bool {
    let children: Vec<View<'a>> = match view.kind {
        Some(SchemaKind::Ref) => return from_ref(converter, stack, view),
        Some(SchemaKind::Array) => view.keyword("items").into_iter().collect(),
        Some(SchemaKind::Object) => view.children("properties").unwrap_or_default(),
        Some(SchemaKind::Tuple) => view.children("items").unwrap_or_default(),
        Some(SchemaKind::Union) => view.children("anyOf").unwrap_or_default(),
        Some(SchemaKind::Intersect) => view.children("allOf").unwrap_or_default(),
        Some(SchemaKind::Record) => view
            .raw
            .get("patternProperties")
            .and_then(Value::as_object)
            .and_then(|patterns| patterns.values().next())
            .map(|value| converter.document.view(value))
            .into_iter()
            .collect(),
        _ => return false,
    };
    children
        .into_iter()
        .any(|child| cyclic_check(converter, stack, child))
}

/// Whether a reference names a definition already on the stack or one that reaches it.
fn from_ref<'a>(converter: &Context<'a>, stack: &mut Vec<&'a str>, view: View<'a>) -> bool {
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
