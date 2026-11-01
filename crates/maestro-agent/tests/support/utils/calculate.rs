//! Calculator tool evaluating one integer addition or multiplication.
use maestro_agent::{AgentTool, AgentToolResult, SharedAgentTool};
use maestro_models::{DiagnosticErrorInfo, TextContent, Tool, UserBlock};
use serde::Deserialize;
use std::sync::{Arc, RwLock};

/// Arguments of the calculator tool.
#[derive(Deserialize)]
pub struct CalculateParams {
    /// Text of the form `<integer> <+ or *> <integer>`.
    pub expression: String,
}

/// Evaluate `expression` and report it as `<expression> = <result>`.
///
/// # Errors
/// Returns the failure text for any other operator, an operand that is not an integer, or an
/// overflow.
pub fn calculate(expression: &str) -> Result<AgentToolResult<()>, String> {
    let unsupported = || format!("Unsupported expression: {expression}");
    let parts: Vec<&str> = expression.split_whitespace().collect();
    let [left, operator, right] = parts[..] else {
        return Err(unsupported());
    };
    let left: i64 = left.parse().map_err(|_| unsupported())?;
    let right: i64 = right.parse().map_err(|_| unsupported())?;
    let result = match operator {
        "+" => left.checked_add(right),
        "*" => left.checked_mul(right),
        _ => return Err(unsupported()),
    };
    let result = result.ok_or_else(|| format!("Arithmetic overflow: {expression}"))?;
    Ok(AgentToolResult {
        content: vec![UserBlock::Text(TextContent::new(format!(
            "{expression} = {result}"
        )))],
        details: None,
        terminate: None,
    })
}

/// The `calculate` tool with its `expression` field and `Calculator` label.
#[must_use]
pub fn calculate_tool() -> SharedAgentTool {
    let definition = Tool {
        name: "calculate".into(),
        description: "Evaluate mathematical expressions".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "expression": {"type": "string", "description": "The mathematical expression to evaluate"}
            },
            "required": ["expression"]
        }).into(),
    };
    let execute = Arc::new(|_, params: CalculateParams, _, _| {
        let result = calculate(&params.expression).map_err(DiagnosticErrorInfo::error);
        Box::pin(std::future::ready(result)) as maestro_models::BoxFuture<_>
    });
    Arc::new(RwLock::new(AgentTool::typed(
        definition,
        "Calculator".into(),
        execute,
    )))
}
