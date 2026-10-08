use num_bigint::BigUint;
use num_traits::ToPrimitive;
use serde_json::Value;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind<'a> {
    Number,
    Integer,
    Boolean,
    String,
    Null,
    Array,
    Object,
    NonJson(&'a str),
    Unknown(&'a str),
}

impl<'a> Kind<'a> {
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

pub(super) fn coerce(value: &mut Value, schema: &Value) {
    if let Some(schemas) = schema.get("allOf").and_then(Value::as_array) {
        for child in schemas {
            coerce(value, child);
        }
    }
    for keyword in ["anyOf", "oneOf"] {
        if let Some(schemas) = schema.get(keyword).and_then(Value::as_array) {
            union(value, schemas);
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
    if kinds.contains(&Kind::Object) {
        object(value, schema);
    }
    if kinds.contains(&Kind::Array) {
        array(value, schema);
    }
}

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
        (Kind::String, Value::Number(number)) => Some(Value::String(
            ryu_js::Buffer::new().format(number.as_f64()?).to_owned(),
        )),
        (Kind::Null, Value::String(text)) if text.is_empty() => Some(Value::Null),
        (Kind::Null, Value::Bool(false)) => Some(Value::Null),
        (Kind::Null, Value::Number(number)) if number.as_f64() == Some(0.0) => Some(Value::Null),
        _ => None,
    }
}

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

fn object(value: &mut Value, schema: &Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let properties = schema.get("properties").and_then(Value::as_object);
    if let Some(properties) = properties {
        for (key, child) in super::diagnostics::entries(properties) {
            if let Some(member) = object.get_mut(key) {
                coerce(member, child);
            }
        }
    }
    if let Some(additional) = schema
        .get("additionalProperties")
        .filter(|schema| schema.is_object())
    {
        for (key, member) in object {
            if !properties.is_some_and(|properties| properties.contains_key(key)) {
                coerce(member, additional);
            }
        }
    }
}

fn array(value: &mut Value, schema: &Value) {
    let (Some(items), Some(values)) = (schema.get("items"), value.as_array_mut()) else {
        return;
    };
    match items {
        Value::Array(schemas) => {
            for (member, child) in values.iter_mut().zip(schemas) {
                coerce(member, child);
            }
        }
        Value::Object(_) => {
            for member in values {
                coerce(member, items);
            }
        }
        _ => {}
    }
}

fn parse_number(text: &str) -> Option<f64> {
    let text = text.trim_matches(numeric_whitespace);
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

fn numeric_whitespace(character: char) -> bool {
    matches!(character,
        '\u{0009}'..='\u{000d}' | ' ' | '\u{00a0}' | '\u{1680}' |
        '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' |
        '\u{205f}' | '\u{3000}' | '\u{feff}'
    )
}

fn union(value: &mut Value, schemas: &[Value]) {
    for child in schemas {
        let mut candidate = value.clone();
        coerce(&mut candidate, child);
        if child.is_object() && super::check::check(child, &candidate).is_empty() {
            *value = candidate;
            break;
        }
    }
}
