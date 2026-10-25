//! Owned primitive conversion and explicitly declared collection traversal.

use super::check::{self, Code};
use crate::DiagnosticErrorInfo;
use num_bigint::BigUint;
use num_traits::ToPrimitive;
use serde_json::Value;

#[derive(Clone, Copy, PartialEq, Eq)]
/// Declared type category controlling conversion and type diagnostics.
pub(super) enum Kind<'a> {
    /// Any JSON number, including a fractional value.
    Number,
    /// A JSON number with no fractional component.
    Integer,
    /// A JSON truth value with the admitted scalar conversions.
    Boolean,
    /// Text accepting admitted number, boolean and null conversions.
    String,
    /// The JSON null value with its admitted empty scalar conversions.
    Null,
    /// A JSON sequence eligible for declared item traversal.
    Array,
    /// A JSON map eligible for declared property traversal.
    Object,
    /// Known runtime type name that no admitted JSON value can satisfy.
    NonJson(&'a str),
    /// Unrecognized type spelling retained as a non-asserting declaration.
    Unknown(&'a str),
}

impl<'a> Kind<'a> {
    /// Classify a declared type while retaining unrecognized names.
    fn parse(name: &'a str) -> Self {
        match name {
            "number" => Self::Number,
            "integer" => Self::Integer,
            "boolean" => Self::Boolean,
            "string" => Self::String,
            "null" => Self::Null,
            "array" => Self::Array,
            "object" => Self::Object,
            "asyncIterator" | "bigint" | "constructor" | "function" | "iterator" | "symbol"
            | "undefined" | "void" => Self::NonJson(name),
            _ => Self::Unknown(name),
        }
    }

    /// Return the declared spelling used in type diagnostics.
    pub(super) fn name(self) -> &'a str {
        match self {
            Self::Number => "number",
            Self::Integer => "integer",
            Self::Boolean => "boolean",
            Self::String => "string",
            Self::Null => "null",
            Self::Array => "array",
            Self::Object => "object",
            Self::NonJson(name) | Self::Unknown(name) => name,
        }
    }
}

/// Apply ordered alternatives and declared conversions to an owned candidate.
pub(super) fn coerce(
    value: &mut Value,
    schema: &Value,
    code: &Code<'_>,
) -> Result<(), DiagnosticErrorInfo> {
    if let Some(schemas) = schema.get("allOf").and_then(Value::as_array) {
        for child in schemas {
            coerce(value, child, code)?;
        }
    }
    for keyword in ["anyOf", "oneOf"] {
        if let Some(schemas) = schema.get(keyword).and_then(Value::as_array) {
            union(value, schemas, code)?;
        }
    }
    let kinds = types(schema);
    if kinds.len() <= 1 || !kinds.iter().any(|kind| matches(value, *kind)) {
        for kind in &kinds {
            if let Some(candidate) = primitive(value, *kind) {
                *value = candidate;
                break;
            }
        }
    }
    let mut result = Ok(());
    let mut visit = |member: &mut Value, child: &Value| {
        if result.is_ok() {
            result = coerce(member, child, code);
        }
    };
    if kinds.contains(&Kind::Object)
        && let Some(object) = value.as_object_mut()
    {
        members(schema, object, &mut visit);
    }
    if kinds.contains(&Kind::Array)
        && let Some(values) = value.as_array_mut()
    {
        if schema.get("items").is_some_and(Value::is_array) {
            positions(schema, values, &mut visit);
        } else {
            sequence(schema, values, &mut visit);
        }
    }
    result
}

/// Read the declared type alternatives without inventing a default.
pub(super) fn types(schema: &Value) -> Vec<Kind<'_>> {
    match schema.get("type") {
        Some(Value::String(kind)) => vec![Kind::parse(kind)],
        Some(Value::Array(kinds)) => kinds
            .iter()
            .filter_map(Value::as_str)
            .map(Kind::parse)
            .collect(),
        _ => Vec::new(),
    }
}

/// Detect already-matching JSON types before attempting union conversion.
pub(super) fn matches(value: &Value, kind: Kind<'_>) -> bool {
    match kind {
        Kind::Number => value.is_number(),
        Kind::Integer => value.as_f64().is_some_and(|number| number.fract() == 0.0),
        Kind::Boolean => value.is_boolean(),
        Kind::String => value.is_string(),
        Kind::Null => value.is_null(),
        Kind::Array => value.is_array(),
        Kind::Object => value.is_object(),
        _ => false,
    }
}

/// Produce an admitted scalar conversion, leaving other kinds untouched.
fn primitive(value: &Value, kind: Kind<'_>) -> Option<Value> {
    match (kind, value) {
        (Kind::Number | Kind::Integer, value) => numeric(value, kind),
        (Kind::Boolean, Value::Null) => Some(Value::Bool(false)),
        (Kind::Boolean, Value::String(text)) if text == "true" || text == "false" => {
            Some(Value::Bool(text == "true"))
        }
        (Kind::Boolean, Value::Number(number)) => match number.as_f64()? {
            0.0 => Some(Value::Bool(false)),
            1.0 => Some(Value::Bool(true)),
            _ => None,
        },
        (Kind::String, Value::Null) => Some(Value::String(String::new())),
        (Kind::String, Value::Bool(boolean)) => Some(Value::String(boolean.to_string())),
        (Kind::String, Value::Number(number)) => number_text(number).map(Value::String),
        (Kind::Null, Value::String(text)) if text.is_empty() => Some(Value::Null),
        (Kind::Null, Value::Bool(false)) => Some(Value::Null),
        (Kind::Null, Value::Number(number)) if number.as_f64() == Some(0.0) => Some(Value::Null),
        _ => None,
    }
}

/// Convert scalar input to a finite number, enforcing integral targets.
fn numeric(value: &Value, kind: Kind<'_>) -> Option<Value> {
    let number = match value {
        Value::Null => 0.0,
        Value::Bool(boolean) => f64::from(u8::from(*boolean)),
        Value::String(text) => parse_number(text)?,
        _ => return None,
    };
    if kind == Kind::Integer && number.fract() != 0.0 {
        return None;
    }
    serde_json::Number::from_f64(number).map(Value::Number)
}

/// Visit every present declared member, then every schema-valued additional member.
///
/// A declared member is visited once, by its exact name; an additional member only when no
/// declared member names it and the declaration supplies a schema for it.
pub(super) fn members<'a>(
    schema: &'a Value,
    object: &mut serde_json::Map<String, Value>,
    mut visit: impl FnMut(&mut Value, &'a Value),
) {
    let properties = schema.get("properties").and_then(Value::as_object);
    if let Some(properties) = properties {
        for (key, child) in super::diagnostics::entries(properties) {
            if let Some(member) = object.get_mut(key) {
                visit(member, child);
            }
        }
    }
    if let Some(additional) = schema
        .get("additionalProperties")
        .filter(|schema| schema.is_object())
    {
        for (key, member) in object {
            if !properties.is_some_and(|properties| properties.contains_key(key)) {
                visit(member, additional);
            }
        }
    }
}

/// Visit the sequence positions the declared positional item nodes cover.
pub(super) fn positions<'a>(
    schema: &'a Value,
    values: &mut [Value],
    mut visit: impl FnMut(&mut Value, &'a Value),
) {
    let Some(schemas) = schema.get("items").and_then(Value::as_array) else {
        return;
    };
    for (member, child) in values.iter_mut().zip(schemas) {
        visit(member, child);
    }
}

/// Visit every sequence member through the single declared item node.
pub(super) fn sequence<'a>(
    schema: &'a Value,
    values: &mut [Value],
    mut visit: impl FnMut(&mut Value, &'a Value),
) {
    let Some(items) = schema.get("items").filter(|items| items.is_object()) else {
        return;
    };
    for member in values {
        visit(member, items);
    }
}

/// Spell a JSON number with the scalar spelling callers compare against.
pub(super) fn number_text(number: &serde_json::Number) -> Option<String> {
    let number = number.as_f64()?;
    Some(ryu_js::Buffer::new().format(number).to_owned())
}

/// Parse nonblank decimal or unsigned radix text through native numeric primitives.
pub(super) fn parse_number(text: &str) -> Option<f64> {
    let text = text.trim_matches(super::super::json_parse::whitespace);
    if text.is_empty() {
        return None;
    }
    let radix = match text.as_bytes().get(..2) {
        Some(b"0x" | b"0X") => Some(16),
        Some(b"0o" | b"0O") => Some(8),
        Some(b"0b" | b"0B") => Some(2),
        _ => None,
    };
    let number = if let Some(radix) = radix {
        let digits = text.get(2..)?;
        if digits.is_empty() || !digits.chars().all(|character| character.is_digit(radix)) {
            return None;
        }
        BigUint::parse_bytes(digits.as_bytes(), radix)?.to_f64()?
    } else {
        text.parse::<f64>().ok()?
    };
    number.is_finite().then_some(number)
}

/// Keep the first independently converted alternative that validates; an alternative whose
/// own expressions do not compile never validates.
fn union(value: &mut Value, schemas: &[Value], code: &Code<'_>) -> Result<(), DiagnosticErrorInfo> {
    for child in schemas {
        let mut candidate = value.clone();
        coerce(&mut candidate, child, code)?;
        if child.is_object() && check::candidate(child, &mut candidate, code)? {
            *value = candidate;
            break;
        }
    }
    Ok(())
}
