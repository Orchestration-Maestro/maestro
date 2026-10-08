//! Compose ordered metadata from native YAML events.

use super::{FrontmatterError, FrontmatterValue};
use std::collections::HashMap;
use yaml_rust2::{
    parser::{Event, MarkedEventReceiver, Parser, Tag},
    scanner::{Marker, TScalarStyle},
};

/// Native parser observations in authored order.
#[derive(Default)]
struct Events(Vec<(Event, Marker)>);
impl MarkedEventReceiver for Events {
    fn on_event(&mut self, event: Event, marker: Marker) {
        self.0.push((event, marker));
    }
}

/// Resolve an extracted metadata document.
pub(super) fn parse(source: &str) -> Result<FrontmatterValue, FrontmatterError> {
    let mut events = Events::default();
    Parser::new_from_str(source)
        .load(&mut events, true)
        .map_err(|error| FrontmatterError {
            message: error.to_string(),
            line: Some(error.marker().line()),
            column: Some(error.marker().col() + 1),
        })?;
    let mut reader = Reader {
        events: events.0.into_iter().peekable(),
        anchors: HashMap::new(),
    };
    while let Some((event, _)) = reader.events.next() {
        if event == Event::DocumentStart {
            let value = reader.value()?;
            if reader
                .events
                .any(|(event, _)| event == Event::DocumentStart)
            {
                return Err(FrontmatterError::new("Source contains multiple documents"));
            }
            return Ok(if value == FrontmatterValue::Null {
                FrontmatterValue::Mapping(Vec::new())
            } else {
                value
            });
        }
    }
    Ok(FrontmatterValue::Mapping(Vec::new()))
}

/// Resolve scalar text according to native style and authored tags.
fn scalar(text: String, style: TScalarStyle, tag: Option<&Tag>) -> FrontmatterValue {
    let tag = tag.filter(|tag| {
        !(tag.handle == "tag:yaml.org,2002:"
            && matches!(
                tag.suffix.as_str(),
                "binary" | "set" | "timestamp" | "omap" | "pairs"
            ))
    });
    if let Some(tag) = tag {
        let resolved = match (tag.handle.as_str(), tag.suffix.as_str()) {
            ("tag:yaml.org,2002:", "int") => integer(&text).map(FrontmatterValue::Number),
            ("tag:yaml.org,2002:", "float") => float(&text).map(FrontmatterValue::Number),
            ("tag:yaml.org,2002:", "bool") => boolean(&text).map(FrontmatterValue::Bool),
            ("tag:yaml.org,2002:", "null") if null(&text) => Some(FrontmatterValue::Null),
            _ => None,
        };
        return resolved.unwrap_or(FrontmatterValue::String(text));
    }
    if style != TScalarStyle::Plain {
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
            return u128::from_str_radix(digits, radix)
                .ok()?
                .to_string()
                .parse()
                .ok();
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

/// Consume native events into owned values.
struct Reader {
    /// Unconsumed authored events.
    events: std::iter::Peekable<std::vec::IntoIter<(Event, Marker)>>,
    /// Completed anchor values available for aliases.
    anchors: HashMap<usize, FrontmatterValue>,
}
impl Reader {
    /// Read one metadata node and register any completed anchor.
    fn value(&mut self) -> Result<FrontmatterValue, FrontmatterError> {
        let (event, _) = self
            .events
            .next()
            .ok_or_else(|| FrontmatterError::new("Missing YAML value"))?;
        let (anchor, value) = match event {
            Event::Scalar(text, style, anchor, tag) => (anchor, scalar(text, style, tag.as_ref())),
            Event::Alias(anchor) => {
                return self
                    .anchors
                    .get(&anchor)
                    .cloned()
                    .ok_or_else(|| FrontmatterError::new("Unresolved YAML alias"));
            }
            Event::SequenceStart(anchor, _) => (anchor, self.sequence()?),
            Event::MappingStart(anchor, _) => (anchor, self.mapping()?),
            _ => return Err(FrontmatterError::new("Expected a YAML value")),
        };
        if anchor != 0 {
            self.anchors.insert(anchor, value.clone());
        }
        Ok(value)
    }

    /// Read an ordered sequence.
    fn sequence(&mut self) -> Result<FrontmatterValue, FrontmatterError> {
        let mut values = Vec::new();
        while !matches!(self.events.peek(), Some((Event::SequenceEnd, _))) {
            values.push(self.value()?);
        }
        self.events.next();
        Ok(FrontmatterValue::Sequence(values))
    }

    /// Read an ordered mapping.
    fn mapping(&mut self) -> Result<FrontmatterValue, FrontmatterError> {
        let mut values: Vec<(String, FrontmatterValue)> = Vec::new();
        let mut keys = Vec::new();
        while !matches!(self.events.peek(), Some((Event::MappingEnd, _))) {
            let key = self.value()?;
            let value = self.value()?;
            let Some(text) = key_text(&key) else {
                continue;
            };
            if keys.iter().any(|existing| existing == &key || matches!((existing, &key), (FrontmatterValue::Number(a), FrontmatterValue::Number(b)) if a.is_nan() && b.is_nan())) {
                return Err(FrontmatterError::new("Duplicated key in mapping"));
            }
            keys.push(key);
            if let Some((_, existing)) = values.iter_mut().find(|(k, _)| k == &text) {
                *existing = value;
            } else {
                values.push((text, value));
            }
        }
        self.events.next();
        Ok(FrontmatterValue::Mapping(values))
    }
}
