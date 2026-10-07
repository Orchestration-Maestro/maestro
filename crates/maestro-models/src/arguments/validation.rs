//! Owned conversion of supplied tool arguments.
use crate::{ThrownValue, Tool, ToolCall};
use serde_json::Value;

/// Select the first exact tool name and validate without executing it.
/// Errors include the requested name and original arguments for correction.
#[doc = include_str!("../../../../docs/models/arguments.md")]
pub fn validate_tool_call(tools: &[Tool], tool_call: &ToolCall) -> Result<Value, ThrownValue> {
    let tool = tools
        .iter()
        .find(|tool| tool.name == tool_call.name)
        .ok_or_else(|| {
            crate::records::diagnostics::error(format!("Tool \"{}\" not found", tool_call.name))
        })?;
    validate_tool_arguments(tool, tool_call)
}
/// Clone, convert and validate against an explicitly supplied declaration.
/// Metadata conversion ignores replacement of a primitive root. Changed plain
/// primitive roots whose candidate fails return the original root without error.
/// Shared schemas cache by identity; neither outcome mutates or executes the call.
pub fn validate_tool_arguments(tool: &Tool, tool_call: &ToolCall) -> Result<Value, ThrownValue> {
    let mut args = crate::scalar::clone_json(&tool_call.arguments);
    let (schema, kinds, legacy) = tool.parameters.snapshot();
    let _ = convert(&mut args, &schema, &kinds, &mut Vec::new());
    let checker = tool.parameters.checker()?;
    if !legacy && record(&schema) {
        let (coerced, changed) = coerce(crate::scalar::clone_json(&args), &schema);
        if changed && !(record(&args) && record(&coerced)) {
            return Ok(if checker.is_valid(&coerced)? {
                coerced
            } else {
                args
            });
        }
        args = coerced;
    }
    if checker.is_valid(&args)? {
        return Ok(args);
    }
    let errors = crate::schema::errors(&schema, &args)?.join("\n");
    let errors = if errors.is_empty() {
        "Unknown validation error"
    } else {
        &errors
    };
    Err(crate::records::diagnostics::error(format!(
        "Validation failed for tool \"{}\":\n{}\n\nReceived arguments:\n{}",
        tool_call.name,
        errors,
        crate::scalar::pretty_json(&tool_call.arguments)
    )))
}
fn convert(
    value: &mut Value,
    schema: &Value,
    kinds: &std::collections::BTreeMap<Vec<String>, String>,
    path: &mut Vec<String>,
) -> Value {
    crate::scalar::grow(|| {
        let kind = kinds
            .get(path)
            .map(String::as_str)
            .or_else(|| schema["~kind"].as_str());
        match kind {
            Some("Object") => {
                if let (Some(object), Some(properties)) =
                    (value.as_object_mut(), schema["properties"].as_object())
                {
                    for (key, nested) in properties {
                        if let Some(v) = object.get_mut(key) {
                            path.extend(["properties".into(), key.clone()]);
                            *v = convert(v, nested, kinds, path);
                            path.truncate(path.len() - 2);
                        }
                    }
                }
                crate::scalar::clone_json(value)
            }
            Some("Tuple") => {
                if let (Some(values), Some(items)) =
                    (value.as_array_mut(), schema["items"].as_array())
                {
                    for (i, v) in values.iter_mut().enumerate().take(items.len()) {
                        path.extend(["items".into(), i.to_string()]);
                        *v = convert(v, &items[i], kinds, path);
                        path.truncate(path.len() - 2);
                    }
                }
                crate::scalar::clone_json(value)
            }
            Some("Union") => {
                let branches = schema["anyOf"].as_array().cloned().unwrap_or_default();
                if branches.iter().any(|branch| {
                    crate::schema::build(branch)
                        .and_then(|c| c.is_valid(value))
                        .is_ok_and(|valid| valid)
                }) {
                    return crate::scalar::clone_json(value);
                }
                for (i, branch) in branches.iter().enumerate() {
                    let mut candidate = crate::scalar::clone_json(value);
                    path.extend(["anyOf".into(), i.to_string()]);
                    candidate = convert(&mut candidate, branch, kinds, path);
                    path.truncate(path.len() - 2);
                    if crate::schema::build(schema)
                        .and_then(|c| c.is_valid(&candidate))
                        .is_ok_and(|valid| valid)
                    {
                        return candidate;
                    }
                }
                crate::scalar::clone_json(value)
            }
            Some("Intersect") => {
                if let Some(branches) = schema["allOf"].as_array() {
                    for (i, branch) in branches.iter().enumerate() {
                        path.extend(["allOf".into(), i.to_string()]);
                        let converted = convert(value, branch, kinds, path);
                        path.truncate(path.len() - 2);
                        if !record(value) {
                            *value = converted;
                        }
                    }
                }
                crate::scalar::clone_json(value)
            }
            Some("Record") => {
                if let (Some(object), Some(patterns)) = (
                    value.as_object_mut(),
                    schema["patternProperties"].as_object(),
                ) {
                    for pattern in crate::schema::keys(patterns) {
                        for (key, v) in object.iter_mut() {
                            if crate::schema::pattern_matches(&pattern, key) {
                                path.extend(["patternProperties".into(), pattern.clone()]);
                                *v = convert(v, &patterns[&pattern], kinds, path);
                                path.truncate(path.len() - 2);
                            }
                        }
                    }
                    if record(&schema["additionalProperties"]) {
                        for (key, v) in object {
                            if !patterns
                                .keys()
                                .any(|p| crate::schema::pattern_matches(p, key))
                            {
                                path.push("additionalProperties".into());
                                *v = convert(v, &schema["additionalProperties"], kinds, path);
                                path.pop();
                            }
                        }
                    }
                }
                crate::scalar::clone_json(value)
            }
            Some("Literal") => {
                let constant = &schema["const"];
                let candidate = metadata_scalar(value, constant);
                if candidate == *constant {
                    candidate
                } else {
                    crate::scalar::clone_json(value)
                }
            }
            Some("Enum") => {
                for constant in schema["enum"].as_array().into_iter().flatten() {
                    if *constant == *value {
                        return crate::scalar::clone_json(value);
                    }
                }
                for constant in schema["enum"].as_array().into_iter().flatten() {
                    let candidate = metadata_scalar(value, constant);
                    if candidate == *constant {
                        return candidate;
                    }
                }
                crate::scalar::clone_json(value)
            }
            Some("TemplateLiteral") => {
                let candidate = if value.is_null() {
                    Value::String("null".into())
                } else {
                    primitive(value, "string").unwrap_or_else(|| crate::scalar::clone_json(value))
                };
                if schema["pattern"].as_str().is_some_and(|p| {
                    crate::schema::pattern_matches(p, candidate.as_str().unwrap_or(""))
                }) {
                    candidate
                } else {
                    crate::scalar::clone_json(value)
                }
            }
            Some("Number" | "Integer") => {
                let candidate = match value {
                    Value::String(s) if crate::scalar::trim(s).is_empty() => Some(0.0),
                    Value::String(s) if s.to_lowercase() == "true" => Some(1.0),
                    Value::String(s) if s.to_lowercase() == "false" => Some(0.0),
                    Value::String(s) => parse_number(s),
                    Value::Number(n) => n.as_f64(),
                    Value::Null => Some(0.0),
                    Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
                    _ => None,
                };
                candidate
                    .and_then(|n| {
                        number(if kind == Some("Integer") {
                            n.trunc()
                        } else {
                            n
                        })
                    })
                    .unwrap_or_else(|| crate::scalar::clone_json(value))
            }
            Some("Boolean") => match value {
                Value::String(s) if s.to_lowercase() == "true" || s == "1" => Value::Bool(true),
                Value::String(s) if s.to_lowercase() == "false" || s == "0" => Value::Bool(false),
                _ => {
                    primitive(value, "boolean").unwrap_or_else(|| crate::scalar::clone_json(value))
                }
            },
            Some("String") => {
                if value.is_null() {
                    Value::String("null".into())
                } else {
                    primitive(value, "string").unwrap_or_else(|| crate::scalar::clone_json(value))
                }
            }
            Some("Null") => match value {
                Value::String(s)
                    if ["null", "undefined", "", "0"].contains(&s.to_lowercase().as_str()) =>
                {
                    Value::Null
                }
                _ => primitive(value, "null").unwrap_or_else(|| crate::scalar::clone_json(value)),
            },
            Some("Array") => {
                let values = if let Value::Array(values) = value {
                    values.clone()
                } else {
                    vec![crate::scalar::clone_json(value)]
                };
                path.push("items".into());
                let result = Value::Array(
                    values
                        .into_iter()
                        .map(|mut v| convert(&mut v, &schema["items"], kinds, path))
                        .collect(),
                );
                path.pop();
                result
            }
            _ => crate::scalar::clone_json(value),
        }
    })
}

fn number(n: f64) -> Option<Value> {
    if !n.is_finite() {
        return None;
    }
    if n.fract() == 0.0 && n >= i64::MIN as f64 && n < -(i64::MIN as f64) {
        Some(Value::from(n as i64))
    } else {
        serde_json::Number::from_f64(n).map(Value::Number)
    }
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

fn record(value: &Value) -> bool {
    value.is_object() || value.is_array()
}
fn coerce(mut value: Value, schema: &Value) -> (Value, bool) {
    crate::scalar::grow(|| {
        let original_primitive = (!record(&value)).then(|| value.clone());
        let mut changed = false;
        if let Some(branches) = schema["allOf"].as_array() {
            for branch in branches {
                let (next, replaced) = coerce(value, branch);
                value = next;
                changed |= replaced;
            }
        }
        for keyword in ["anyOf", "oneOf"] {
            if let Some(branches) = schema[keyword].as_array() {
                for branch in branches {
                    let (candidate, replaced) = coerce(crate::scalar::clone_json(&value), branch);
                    if record(branch)
                        && crate::schema::build(branch)
                            .and_then(|checker| checker.is_valid(&candidate))
                            .is_ok_and(|valid| valid)
                    {
                        changed |= replaced || record(&candidate);
                        value = candidate;
                        break;
                    }
                }
            }
        }
        let types: Vec<&str> = match &schema["type"] {
            Value::String(t) => vec![t],
            Value::Array(ts) => ts.iter().filter_map(Value::as_str).collect(),
            _ => vec![],
        };
        if !(types.len() > 1 && types.iter().any(|kind| matches_type(&value, kind))) {
            for kind in &types {
                if let Some(candidate) = primitive(&value, kind)
                    && candidate != value
                {
                    value = candidate;
                    changed = true;
                    break;
                }
            }
        }
        if types.contains(&"object")
            && let Some(object) = value.as_object_mut()
        {
            if let Some(properties) = schema["properties"].as_object() {
                for key in crate::schema::keys(properties) {
                    if let Some(v) = object.get_mut(&key) {
                        *v = coerce(crate::scalar::clone_json(v), &properties[&key]).0;
                    }
                }
            }
            if record(&schema["additionalProperties"]) {
                for (key, v) in object {
                    if schema["properties"].get(key).is_none() {
                        *v = coerce(
                            crate::scalar::clone_json(v),
                            &schema["additionalProperties"],
                        )
                        .0;
                    }
                }
            }
        }
        if types.contains(&"array")
            && let Some(items) = value.as_array_mut()
        {
            for (i, v) in items.iter_mut().enumerate() {
                let nested = if let Some(tuple) = schema["items"].as_array() {
                    tuple.get(i)
                } else {
                    record(&schema["items"]).then_some(&schema["items"])
                };
                if let Some(nested) = nested {
                    *v = coerce(crate::scalar::clone_json(v), nested).0;
                }
            }
        }
        let changed = original_primitive.map_or(changed, |original| original != value);
        (value, changed)
    })
}

fn metadata_scalar(value: &Value, constant: &Value) -> Value {
    let kind = if constant.is_number() {
        "Number"
    } else if constant.is_boolean() {
        "Boolean"
    } else {
        "String"
    };
    let mut value = crate::scalar::clone_json(value);
    convert(
        &mut value,
        &serde_json::json!({"~kind":kind}),
        &Default::default(),
        &mut Vec::new(),
    )
}
