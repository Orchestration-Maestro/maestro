//! Owned conversion and corrective diagnostics for tool invocations.

mod authored;
mod check;
mod coercion;
mod collections;
mod diagnostics;
mod formats;
mod prepare;
mod references;
mod scalars;
mod schema;

use crate::{DiagnosticErrorInfo, SchemaCodeSource, Tool, ToolCall};
use serde_json::Value;

/// Select the first declaration with the invocation's exact name and check its arguments.
///
/// # Errors
/// Returns a corrective diagnostic when no declaration matches, the schema holds a malformed
/// regular expression or the arguments are invalid.
pub fn validate_tool_call<C: SchemaCodeSource + Default>(
    tools: &[Tool<C>],
    tool_call: &ToolCall,
) -> Result<Value, DiagnosticErrorInfo> {
    let tool = tools
        .iter()
        .find(|tool| tool.name == tool_call.name)
        .ok_or_else(|| diagnostic(format!("Tool \"{}\" not found", tool_call.name)))?;
    validate_tool_arguments(tool, tool_call)
}

/// Convert an owned argument candidate and check it against the supplied declaration.
///
/// The invocation and declaration are never modified. Declared conversions and supplied
/// callbacks operate on an owned candidate; their in-place edits reach later checks.
/// References resolve offline against the original schema, without resource retrieval.
///
/// # Errors
/// Returns at most eight corrective messages in evaluation order. Ordinary duplicates
/// are suppressed; separate refinements may retain equal messages. Author errors propagate
/// unchanged. Corrective diagnostics are
/// followed by the original, unconverted arguments. A regular expression that the schema
/// builds and that does not compile is a schema error: the diagnostic carries the expression
/// error alone, whatever the arguments are.
pub fn validate_tool_arguments<C: SchemaCodeSource + Default>(
    tool: &Tool<C>,
    tool_call: &ToolCall,
) -> Result<Value, DiagnosticErrorInfo> {
    let schema = tool.parameters.as_json();
    let mut candidate = tool_call.arguments.clone();
    let code = check::Code::new(schema, tool.parameters.code().schema_code());
    authored::convert(&mut candidate, schema, tool.parameters.conversions(), &code)?;
    coercion::coerce(&mut candidate, schema, &code)?;
    let (candidate, errors) = check::check_owned(schema, candidate, &code)?;
    if errors.is_empty() {
        return Ok(candidate);
    }
    Err(diagnostic(format!(
        "Validation failed for tool \"{}\":\n{}\n\nReceived arguments:\n{}",
        tool_call.name,
        errors.join("\n"),
        diagnostics::pretty(&tool_call.arguments).map_err(|error| diagnostic(error.to_string()))?
    )))
}

/// Check the original JSON value without converting it or changing the schema.
///
/// # Errors
/// Returns the existing diagnostic when schema preparation fails.
pub fn validate_schema(schema: &Value, value: &Value) -> Result<Vec<String>, DiagnosticErrorInfo> {
    check::check(schema, &[], value)
}

/// Wrap a corrective message in the existing error record.
fn diagnostic(message: String) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo::error(message)
}
