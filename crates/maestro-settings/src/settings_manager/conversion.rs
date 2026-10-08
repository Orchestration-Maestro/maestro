//! Stored-format conversions, the one-level merge and the saved JSON text.

use std::io;

use serde::Serialize;
use serde_json::ser::{Formatter, PrettyFormatter};
use serde_json::{Map, Serializer, Value};

/// Largest magnitude below which every integral `f64` is an exact integer.
const EXACT_INTEGER_LIMIT: f64 = 9_007_199_254_740_992.0;

/// Writes a typed number as JSON; integral values become integers and
/// non-finite values become `null`.
pub(crate) fn number(value: f64) -> Value {
    if value == 0.0 {
        return Value::from(0);
    }
    if value.is_finite() && value.fract() == 0.0 && value.abs() < EXACT_INTEGER_LIMIT {
        // An integral value below 2^53 prints without a fraction and parses back exactly.
        if let Ok(integer) = format!("{value}").parse::<i64>() {
            return Value::from(integer);
        }
    }
    serde_json::Number::from_f64(value).map_or(Value::Null, Value::Number)
}

/// Merges `over` onto `base`: object pairs merge one level, everything else
/// in `over` replaces.
pub(crate) fn merge_one_level(
    mut base: Map<String, Value>,
    over: Map<String, Value>,
) -> Map<String, Value> {
    for (key, value) in over {
        match (base.get_mut(&key), value) {
            (Some(Value::Object(inner)), Value::Object(outer)) => inner.extend(outer),
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
    base
}

/// Parses stored text into a converted document; empty text is an empty document.
pub(crate) fn parse_document(text: &str) -> Result<Map<String, Value>, serde_json::Error> {
    if text.is_empty() {
        return Ok(Map::new());
    }
    let mut document: Map<String, Value> = serde_json::from_str(text)?;
    convert(&mut document);
    Ok(document)
}

/// Applies the four stored-format conversions in place.
pub(crate) fn convert(document: &mut Map<String, Value>) {
    if !document.contains_key("steeringMode")
        && let Some(legacy) = document.shift_remove("queueMode")
    {
        document.insert("steeringMode".to_owned(), legacy);
    }
    convert_transport(document);
    convert_skills(document);
    convert_retry_delay(document);
}

/// Turns a legacy boolean `websockets` into `transport` when it is absent.
fn convert_transport(document: &mut Map<String, Value>) {
    if document.contains_key("transport") {
        return;
    }
    if let Some(websockets) = document.get("websockets").and_then(Value::as_bool) {
        document.shift_remove("websockets");
        let transport = if websockets { "websocket" } else { "sse" };
        document.insert("transport".to_owned(), transport.into());
    }
}

/// Turns the legacy `skills` object into a command preference and directory list.
fn convert_skills(document: &mut Map<String, Value>) {
    let Some(Value::Object(skills)) = document.get_mut("skills") else {
        return;
    };
    let command = skills.shift_remove("enableSkillCommands");
    let directories = skills
        .shift_remove("customDirectories")
        .filter(|directories| {
            directories
                .as_array()
                .is_some_and(|items| !items.is_empty())
        });
    if let Some(command) = command
        && !document.contains_key("enableSkillCommands")
    {
        document.insert("enableSkillCommands".to_owned(), command);
    }
    match directories {
        Some(directories) => document.insert("skills".to_owned(), directories),
        None => document.shift_remove("skills"),
    };
}

/// Moves a numeric `retry.maxDelayMs` into an absent or null provider delay.
fn convert_retry_delay(document: &mut Map<String, Value>) {
    let Some(Value::Object(retry)) = document.get_mut("retry") else {
        return;
    };
    let Some(delay @ Value::Number(_)) = retry.shift_remove("maxDelayMs") else {
        return;
    };
    if let Some(Value::Object(provider)) = retry.get_mut("provider") {
        if provider.get("maxRetryDelayMs").is_none_or(Value::is_null) {
            provider.insert("maxRetryDelayMs".to_owned(), delay);
        }
    } else {
        let provider = Map::from_iter([("maxRetryDelayMs".to_owned(), delay)]);
        retry.insert("provider".to_owned(), Value::Object(provider));
    }
}

/// Renders a document with two-space indentation and no final newline.
pub(crate) fn to_text(document: &Map<String, Value>) -> Result<String, serde_json::Error> {
    let mut output = Vec::new();
    let formatter = ScriptNumbers(PrettyFormatter::with_indent(b"  "));
    document.serialize(&mut Serializer::with_formatter(&mut output, formatter))?;
    String::from_utf8(output).map_err(serde::ser::Error::custom)
}

/// Pretty formatting that spells floats the way JSON producers conventionally
/// do: `1` rather than `1.0`, and `1e+21` for large magnitudes.
struct ScriptNumbers<'a>(PrettyFormatter<'a>);

impl Formatter for ScriptNumbers<'_> {
    fn write_f64<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: f64) -> io::Result<()> {
        writer.write_all(ryu_js::Buffer::new().format_finite(value).as_bytes())
    }

    fn begin_array<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.begin_array(writer)
    }

    fn end_array<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.end_array(writer)
    }

    fn begin_array_value<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.0.begin_array_value(writer, first)
    }

    fn end_array_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.end_array_value(writer)
    }

    fn begin_object<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.begin_object(writer)
    }

    fn end_object<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.end_object(writer)
    }

    fn begin_object_key<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.0.begin_object_key(writer, first)
    }

    fn begin_object_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.begin_object_value(writer)
    }

    fn end_object_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.end_object_value(writer)
    }
}
