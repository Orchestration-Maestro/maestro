//! Conversion through the constants an enumerated node declares.

use serde_json::Value;

use super::from_union::from_union;
use crate::parameters::validation::authored::context::Context;
use crate::parameters::validation::authored::schema_view::View;
use crate::parameters::validation::authored::types::engine::enumeration::enum_to_union;

/// Convert through the alternatives the enumerated constants name, in declaration order.
pub(crate) fn from_enum<'a>(
    converter: &mut Context<'a>,
    view: View<'a>,
    value: &mut Value,
) -> Option<Value> {
    let constants = view.raw.get("enum").and_then(Value::as_array)?;
    let alternatives = enum_to_union(constants);
    from_union(converter, &alternatives, None, value)
}
