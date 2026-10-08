//! Owned conversion and corrective diagnostics for tool invocations.

mod check;
mod coercion;
mod collections;
mod diagnostics;
mod formats;
mod prepare;
mod references;
mod scalars;

use crate::{DiagnosticErrorInfo, JsonObject, Tool, ToolCall};
use serde_json::Value;

/// Select the first declaration with the invocation's exact name and check its arguments.
///
/// # Errors
/// Returns a corrective diagnostic when no declaration matches, the schema holds a malformed
/// regular expression or the arguments are invalid.
pub fn validate_tool_call(
    tools: &[Tool],
    tool_call: &ToolCall,
) -> Result<JsonObject, DiagnosticErrorInfo> {
    let tool = tools
        .iter()
        .find(|tool| tool.name == tool_call.name)
        .ok_or_else(|| diagnostic(format!("Tool \"{}\" not found", tool_call.name)))?;
    validate_tool_arguments(tool, tool_call)
}

/// Convert an owned argument candidate and check it against the supplied declaration.
///
/// The invocation and declaration are never modified. Only declared primitive conversions
/// apply; missing properties are not inserted and unknown properties are not removed.
/// References resolve offline against the original schema, without resource retrieval.
///
/// # Errors
/// Returns at most eight distinct corrective messages in evaluation order,
/// followed by the original, unconverted arguments. A regular expression that the schema
/// builds and that does not compile is a schema error: the diagnostic carries the expression
/// error alone, whatever the arguments are.
pub fn validate_tool_arguments(
    tool: &Tool,
    tool_call: &ToolCall,
) -> Result<JsonObject, DiagnosticErrorInfo> {
    let mut candidate = Value::Object(tool_call.arguments.clone());
    coercion::coerce(&mut candidate, &tool.parameters);
    let errors = check::check(&tool.parameters, &candidate)
        .map_err(|error| diagnostic(error.to_string()))?;
    if errors.is_empty()
        && let Value::Object(arguments) = candidate
    {
        return Ok(arguments);
    }
    Err(diagnostic(format!(
        "Validation failed for tool \"{}\":\n{}\n\nReceived arguments:\n{}",
        tool_call.name,
        errors.join("\n"),
        diagnostics::pretty(&tool_call.arguments).map_err(|error| diagnostic(error.to_string()))?
    )))
}

/// Wrap a corrective message in the existing error record.
fn diagnostic(message: String) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        name: Some("Error".to_owned()),
        message,
        stack: None,
        code: None,
    }
}
