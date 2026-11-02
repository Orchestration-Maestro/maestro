//! Conversion through the constants an enumerated node declares.

use crate::DiagnosticErrorInfo;
use serde_json::Value;

use super::from_union::from_union;
use crate::arguments::validation::authored::context::Context;
use crate::arguments::validation::authored::schema_view::View;
use crate::arguments::validation::authored::types::engine::enumeration::enum_to_union;

/// Convert through the alternatives the enumerated constants name, in declaration order.
pub(crate) fn from_enum<'a>(
    converter: &mut Context<'a>,
    view: View<'a>,
    value: &mut Value,
) -> Result<Option<Value>, DiagnosticErrorInfo> {
    let Some(constants) = view.raw.get("enum").and_then(Value::as_array) else {
        return Ok(None);
    };
    let alternatives = enum_to_union(constants);
    from_union(converter, &alternatives, None, value)
}
