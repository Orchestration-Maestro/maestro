//! Pure tool declaration lookup with owned, validated argument objects.

use crate::{ToolCall, ToolDeclaration};
use serde_json::{Map, Value};

/// Safe validation outcomes, without raw names, schemas or argument values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolValidationError {
    /// No declaration matches the call name.
    UnknownTool,
    /// The call has no completed object arguments.
    IncompleteArguments,
    /// The schema is invalid or cannot be resolved offline.
    InvalidSchema,
    /// The argument object does not satisfy the schema.
    InvalidArguments,
}
impl std::fmt::Display for ToolValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::UnknownTool => "unknown tool declaration",
            Self::IncompleteArguments => "incomplete tool arguments",
            Self::InvalidSchema => "invalid or unresolvable tool schema",
            Self::InvalidArguments => "invalid tool arguments",
        })
    }
}
impl std::error::Error for ToolValidationError {}

/// Find the first matching declaration and validate an independent argument object.
/// Completed arguments are not authorization. This function never executes tools,
/// mutates inputs, or retrieves schemas from the network or filesystem.
/// Numeric strings trim exactly ECMAScript whitespace; number-to-string coercion
/// uses binary64 shortest scalar spelling. Already accepted numbers remain exact.
/// Invalid schemas and unavailable references fail closed with safe typed errors.
pub fn validate_tool_call(
    tools: &[ToolDeclaration],
    call: &ToolCall,
) -> Result<Map<String, Value>, ToolValidationError> {
    let tool = tools
        .iter()
        .find(|tool| tool.name == call.name)
        .ok_or(ToolValidationError::UnknownTool)?;
    let arguments = call
        .arguments()
        .ok_or(ToolValidationError::IncompleteArguments)?
        .clone();
    let checker = crate::schema::Schema::build(&tool.parameters)?;
    let mut value = Value::Object(arguments);
    coerce(&mut value, &tool.parameters, &checker, "#")?;
    if !checker.is_valid(&value) {
        return Err(ToolValidationError::InvalidArguments);
    }
    match value {
        Value::Object(arguments) => Ok(arguments),
        _ => Err(ToolValidationError::InvalidArguments),
    }
}

fn coerce(
    value: &mut Value,
    schema: &Value,
    checker: &crate::schema::Schema,
    path: &str,
) -> Result<(), ToolValidationError> {
    if let Some(branches) = schema["allOf"].as_array() {
        for (index, branch) in branches.iter().enumerate() {
            coerce(value, branch, checker, &format!("{path}/allOf/{index}"))?;
        }
    }
    for keyword in ["anyOf", "oneOf"] {
        if let Some(branches) = schema[keyword].as_array() {
            for (index, branch) in branches.iter().enumerate() {
                let branch_path = format!("{path}/{keyword}/{index}");
                let mut candidate = value.clone();
                coerce(&mut candidate, branch, checker, &branch_path)?;
                if checker.branch_is_valid(&branch_path, &candidate)? {
                    *value = candidate;
                    break;
                }
            }
        }
    }
    let types: Vec<&str> = match &schema["type"] {
        Value::String(kind) => vec![kind],
        Value::Array(types) => types.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    };
    if !types.iter().any(|kind| matches_type(value, kind)) {
        for kind in &types {
            if let Some(candidate) = primitive(value, kind) {
                *value = candidate;
                break;
            }
        }
    }
    if types.contains(&"object")
        && let Value::Object(object) = value
    {
        for (key, value) in object {
            if let Some(nested) = schema["properties"].get(key) {
                coerce(
                    value,
                    nested,
                    checker,
                    &format!(
                        "{path}/properties/{}",
                        key.replace('~', "~0").replace('/', "~1")
                    ),
                )?;
            } else if schema["additionalProperties"].is_object() {
                coerce(
                    value,
                    &schema["additionalProperties"],
                    checker,
                    &format!("{path}/additionalProperties"),
                )?;
            }
        }
    }
    if types.contains(&"array")
        && let Value::Array(items) = value
    {
        for (index, value) in items.iter_mut().enumerate() {
            match &schema["items"] {
                Value::Array(tuple) => {
                    if let Some(nested) = tuple.get(index) {
                        coerce(value, nested, checker, &format!("{path}/items/{index}"))?;
                    }
                }
                Value::Object(_) => {
                    coerce(value, &schema["items"], checker, &format!("{path}/items"))?
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn matches_type(value: &Value, kind: &str) -> bool {
    match kind {
        "number" => value.is_number(),
        "integer" => value.as_f64().is_some_and(|number| number.fract() == 0.0),
        "boolean" => value.is_boolean(),
        "string" => value.is_string(),
        "null" => value.is_null(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        _ => false,
    }
}

fn primitive(value: &Value, kind: &str) -> Option<Value> {
    match kind {
        "number" | "integer" => {
            let number = match value {
                Value::Null => 0.0,
                Value::Bool(boolean) => {
                    if *boolean {
                        1.0
                    } else {
                        0.0
                    }
                }
                Value::String(text) => parse_number(text)?,
                _ => return None,
            };
            if kind == "integer" && number.fract() != 0.0 {
                return None;
            }
            if number >= i64::MIN as f64 && number < -(i64::MIN as f64) && number.fract() == 0.0 {
                return Some(Value::from(number as i64));
            }
            serde_json::Number::from_f64(number).map(Value::Number)
        }
        "boolean" => match value {
            Value::Null => Some(Value::Bool(false)),
            Value::String(text) if text == "true" => Some(Value::Bool(true)),
            Value::String(text) if text == "false" => Some(Value::Bool(false)),
            Value::Number(number) if number.as_f64() == Some(1.0) => Some(Value::Bool(true)),
            Value::Number(number) if number.as_f64() == Some(0.0) => Some(Value::Bool(false)),
            _ => None,
        },
        "string" => match value {
            Value::Null => Some(Value::String(String::new())),
            Value::Bool(boolean) => Some(Value::String(boolean.to_string())),
            Value::Number(number) => crate::scalar::number_string(number).map(Value::String),
            _ => None,
        },
        "null" if value == "" || value == &Value::Bool(false) || value.as_f64() == Some(0.0) => {
            Some(Value::Null)
        }
        _ => None,
    }
}

fn parse_number(text: &str) -> Option<f64> {
    let text = crate::scalar::trim(text);
    if text.is_empty() {
        return None;
    }
    let prefix = text.get(..2);
    let radix = match prefix {
        Some("0x" | "0X") => Some(16),
        Some("0o" | "0O") => Some(8),
        Some("0b" | "0B") => Some(2),
        _ => None,
    };
    let number = if let Some(radix) = radix {
        let digits = &text[2..];
        if digits.is_empty() {
            return None;
        }
        digits.chars().try_fold(0.0, |value, digit| {
            digit
                .to_digit(radix)
                .map(|digit| value * f64::from(radix) + f64::from(digit))
        })?
    } else {
        text.parse::<f64>().ok()?
    };
    number.is_finite().then_some(number)
}
