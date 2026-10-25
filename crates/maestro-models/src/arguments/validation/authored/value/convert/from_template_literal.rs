//! Conversion through the literals a template pattern names.

use crate::DiagnosticErrorInfo;
use serde_json::Value;

use super::from_string::from_string;
use super::from_type::from_type;
use crate::arguments::validation::authored::context::Context;
use crate::arguments::validation::authored::schema_view::View;
use crate::arguments::validation::authored::types::engine::template_literal::decode::decode;

/// Convert through the literals the declared template pattern names.
///
/// A finite pattern converts through its decoded node; any other pattern stands for text, as
/// the reference decoder yields the string type for it.
pub(crate) fn from_template_literal<'a>(
    converter: &mut Context<'a>,
    view: View<'a>,
    value: &mut Value,
) -> Result<Option<Value>, DiagnosticErrorInfo> {
    let Some(pattern) = view.raw.get("pattern").and_then(Value::as_str) else {
        return Ok(None);
    };
    match decode(pattern) {
        Some(decoded) => from_type(converter, &decoded, value),
        None => Ok(from_string(value)),
    }
}
