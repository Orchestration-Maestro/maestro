//! Validated semantic operands shared by cache equality and browser presentation.
use super::super::document::invalid;
use maestro_app::{is_truthy, normalize_to_lf};
use serde_json::Value;
use std::{borrow::Cow, io};

/// Selected normalized operands used by retained header equality.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Admitted {
    /// Normalized system prompt and its original line boundary.
    pub(super) prompt: Option<Prompt>,
    /// Selected normalized tool operands in supplied order.
    pub(super) tools: Vec<Tool>,
}
impl Admitted {
    /// Validate prompt and tools once, retaining only selected semantic content.
    pub(super) fn new(data: &serde_json::Map<String, Value>) -> io::Result<Self> {
        Ok(Self {
            prompt: prompt(data.get("systemPrompt").unwrap_or(&Value::Null))?,
            tools: tools(data.get("tools").unwrap_or(&Value::Null))?,
        })
    }
}
/// Complete normalized prompt content with its preview boundary.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Prompt {
    /// A short prompt with no disclosure handler.
    Full(String),
    /// A long prompt with its original LF-based line count.
    Preview {
        /// Full native text.
        full: String,
        /// First ten authored LF-separated lines.
        preview: String,
        /// Original lines remaining after the preview.
        remaining: usize,
    },
}
/// Project literal text as parsed native text nodes.
fn literal(text: &str) -> String {
    normalize_to_lf(text).replace('\0', "")
}
/// Prepare the reached prompt operand before tool operations.
fn prompt(value: &Value) -> io::Result<Option<Prompt>> {
    if !is_truthy(value) {
        return Ok(None);
    }
    let text = value
        .as_str()
        .ok_or_else(|| invalid("nonstring system prompt"))?;
    let lines: Vec<_> = text.split('\n').collect();
    let full = literal(text);
    Ok(Some(if lines.len() > 10 {
        Prompt::Preview {
            full,
            preview: literal(&lines[..10].join("\n")),
            remaining: lines.len() - 10,
        }
    } else {
        Prompt::Full(full)
    }))
}

/// Literal tool with optional parameter rows.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Tool {
    /// Displayed tool name.
    pub name: String,
    /// Untrimmed description.
    pub description: String,
    /// Prepared even when initially collapsed.
    pub parameters: Vec<Parameter>,
}
/// Read an optional literal display field without runtime coercion.
fn display(value: &Value) -> io::Result<String> {
    if value.is_null() {
        return Ok(String::new());
    }
    value
        .as_str()
        .map(literal)
        .ok_or_else(|| invalid("nonstring display operand"))
}
/// Visit tools in supplied order, rejecting only reached mapping operations.
fn tools(value: &Value) -> io::Result<Vec<Tool>> {
    match value {
        Value::Array(tools) => tools
            .iter()
            .map(|tool| {
                if tool.is_null() {
                    return Err(invalid("null tool receiver"));
                }
                let parameters = parameters(tool.get("parameters").unwrap_or(&Value::Null))?;
                Ok(Tool {
                    parameters,
                    name: display(&tool["name"])?,
                    description: display(&tool["description"])?,
                })
            })
            .collect(),
        Value::String(text) if !text.is_empty() => Err(invalid("tools cannot be mapped")),
        Value::Object(fields)
            if fields
                .get("length")
                .and_then(Value::as_f64)
                .is_some_and(|n| n > 0.) =>
        {
            Err(invalid("tools cannot be mapped"))
        }
        _ => Ok(Vec::new()),
    }
}

/// A schema entry retaining display and exact required membership separately.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Parameter {
    /// Literal display name, not the required-membership identity.
    pub name: String,
    /// Literal union or fallback type.
    pub kind: String,
    /// Exact raw-string membership decision.
    pub required: bool,
    /// Optional untrimmed description.
    pub description: Option<String>,
}
/// Enumerate only supported schema collections through the existing key owner.
fn properties(value: &Value) -> Vec<(Cow<'_, str>, &Value)> {
    match value {
        Value::Object(fields) => maestro_app::canonical_entries(
            fields.iter().map(|(k, v)| (Cow::Borrowed(k.as_str()), v)),
        ),
        Value::Array(values) => values
            .iter()
            .enumerate()
            .map(|(i, v)| (Cow::Owned(i.to_string()), v))
            .collect(),
        _ => Vec::new(),
    }
}
/// Prepare parameter operations in their observable failure order.
fn parameters(schema: &Value) -> io::Result<Vec<Parameter>> {
    let props = properties(&schema["properties"]);
    if props.is_empty() {
        return Ok(Vec::new());
    }
    let required = &schema["required"];
    let required = if is_truthy(required) {
        required
            .as_array()
            .ok_or_else(|| invalid("invalid required list"))?
            .as_slice()
    } else {
        &[]
    };
    props
        .into_iter()
        .map(|(name, prop)| {
            let required = required.iter().any(|v| v.as_str() == Some(name.as_ref()));
            if prop.is_null() {
                return Err(invalid("null property receiver"));
            }
            let kind = parameter_type(&prop["type"])?;
            let description = is_truthy(&prop["description"])
                .then(|| display(&prop["description"]))
                .transpose()?;
            Ok(Parameter {
                name: literal(&name),
                kind,
                required,
                description,
            })
        })
        .collect()
}
/// Format admitted string unions without replacing an empty union with fallback.
fn parameter_type(value: &Value) -> io::Result<String> {
    if !is_truthy(value) {
        return Ok("any".into());
    }
    match value {
        Value::Array(values) => values
            .iter()
            .map(|v| v.as_str().ok_or_else(|| invalid("nonstring union member")))
            .collect::<io::Result<Vec<_>>>()
            .map(|v| literal(&v.join(","))),
        _ => display(value),
    }
}
