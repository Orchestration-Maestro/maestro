//! Resolve core scalars and construct ordered typed-key mappings.

use super::{FrontmatterError, FrontmatterValue, resolve_flow_scalar::Style, resolve_props::Tag};
use num_traits::ToPrimitive;

/// Resolve scalar text according to native style and authored tags.
pub(super) fn scalar(text: String, style: Style, tag: Option<&Tag>) -> FrontmatterValue {
    if let Some(tag) = tag.filter(|tag| **tag != Tag::Data) {
        let resolved = match tag {
            Tag::Int => integer(&text).map(FrontmatterValue::Number),
            Tag::Float => float(&text).map(FrontmatterValue::Number),
            Tag::Bool => boolean(&text).map(FrontmatterValue::Bool),
            Tag::Null if null(&text) => Some(FrontmatterValue::Null),
            _ => None,
        };
        return resolved.unwrap_or(FrontmatterValue::String(text));
    }
    if style != Style::Plain {
        return FrontmatterValue::String(text);
    }
    if null(&text) {
        return FrontmatterValue::Null;
    }
    boolean(&text)
        .map(FrontmatterValue::Bool)
        .or_else(|| {
            integer(&text)
                .or_else(|| float(&text))
                .map(FrontmatterValue::Number)
        })
        .unwrap_or(FrontmatterValue::String(text))
}

/// Recognize core null spellings.
fn null(text: &str) -> bool {
    matches!(text, "" | "~" | "null" | "Null" | "NULL")
}

/// Recognize core boolean spellings.
fn boolean(text: &str) -> Option<bool> {
    match text {
        "true" | "True" | "TRUE" => Some(true),
        "false" | "False" | "FALSE" => Some(false),
        _ => None,
    }
}

/// Resolve decimal, hexadecimal and octal integers.
fn integer(text: &str) -> Option<f64> {
    for (prefix, radix) in [("0x", 16), ("0o", 8)] {
        if let Some(digits) = text.strip_prefix(prefix) {
            if digits.is_empty() || !digits.chars().all(|digit| digit.is_digit(radix)) {
                return None;
            }
            return num_bigint::BigUint::parse_bytes(digits.as_bytes(), radix)?.to_f64();
        }
    }
    let digits = text.strip_prefix(['-', '+']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// Resolve core fractional, exponent and nonfinite scalars.
fn float(text: &str) -> Option<f64> {
    match text {
        ".inf" | ".Inf" | ".INF" | "+.inf" | "+.Inf" | "+.INF" => return Some(f64::INFINITY),
        "-.inf" | "-.Inf" | "-.INF" => return Some(f64::NEG_INFINITY),
        ".nan" | ".NaN" | ".NAN" => return Some(f64::NAN),
        _ => (),
    }
    if !text.contains(['.', 'e', 'E']) {
        return None;
    }
    text.parse().ok()
}

/// Convert resolved scalar keys to their observable spelling.
fn key_text(value: &FrontmatterValue) -> Option<String> {
    match value {
        FrontmatterValue::Null => Some(String::new()),
        FrontmatterValue::Bool(value) => Some(value.to_string()),
        FrontmatterValue::Number(value) => Some(ryu_js::Buffer::new().format(*value).into()),
        FrontmatterValue::String(text) => Some(text.clone()),
        _ => None,
    }
}

/// One mapping's typed duplicate keys and observable string-keyed members.
#[derive(Default)]
pub(super) struct Mapping {
    /// Scalar keys before their string projection.
    keys: Vec<FrontmatterValue>,
    /// First insertion order with last distinct-kind value replacement.
    values: Vec<(String, FrontmatterValue)>,
}

impl Mapping {
    /// Validate a complete pair before exposing its projected value.
    pub fn insert(
        &mut self,
        key: FrontmatterValue,
        value: FrontmatterValue,
    ) -> Result<(), FrontmatterError> {
        let Some(text) = key_text(&key) else {
            return Ok(());
        };
        if self.keys.iter().any(|existing| existing == &key || matches!((existing, &key), (FrontmatterValue::Number(a), FrontmatterValue::Number(b)) if a.is_nan() && b.is_nan())) {
            return Err(FrontmatterError::new("Duplicated key in mapping"));
        }
        self.keys.push(key);
        if let Some((_, existing)) = self.values.iter_mut().find(|(k, _)| k == &text) {
            *existing = value;
        } else {
            self.values.push((text, value));
        }
        Ok(())
    }

    /// Return the completed ordered mapping.
    pub fn finish(self) -> FrontmatterValue {
        FrontmatterValue::Mapping(self.values)
    }
}
