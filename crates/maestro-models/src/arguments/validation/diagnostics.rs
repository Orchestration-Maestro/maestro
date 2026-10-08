//! Canonical argument serialization and shared diagnostic path display.

use std::io::{self, Write};

use serde::{
    Serialize, Serializer,
    ser::{SerializeMap, SerializeSeq},
};
use serde_json::{
    Map, Value,
    ser::{Formatter, PrettyFormatter},
};

/// Serialize original arguments with canonical keys and binary64 number spelling.
pub(super) fn pretty(value: &Map<String, Value>) -> Result<String, serde_json::Error> {
    let mut serializer =
        serde_json::Serializer::with_formatter(Vec::new(), NumberFormatter(PrettyFormatter::new()));
    CanonicalObject(value).serialize(&mut serializer)?;
    String::from_utf8(serializer.into_inner())
        .map_err(|error| serde_json::Error::io(io::Error::new(io::ErrorKind::InvalidData, error)))
}

/// Enumerate numeric-index keys first while preserving other insertion order.
pub(super) fn entries(object: &Map<String, Value>) -> Vec<(&String, &Value)> {
    let mut entries: Vec<_> = object.iter().collect();
    entries.sort_by_key(|(key, _)| index(key).map_or((1, 0), |index| (0, index)));
    entries
}

/// Display instance separators and append a literal nonempty missing name.
pub(super) fn path(instance: &str, missing: Option<&str>) -> String {
    let mut display = instance
        .strip_prefix('/')
        .unwrap_or(instance)
        .replace('/', ".");
    if let Some(name) = missing.filter(|name| !name.is_empty()) {
        if !display.is_empty() {
            display.push('.');
        }
        display.push_str(name);
    }
    if display.is_empty() {
        "root".into()
    } else {
        display
    }
}

/// Recognize canonical numeric-index keys below the reserved upper bound.
fn index(key: &str) -> Option<u32> {
    let index = key.parse::<u32>().ok()?;
    (index < u32::MAX && index.to_string() == key).then_some(index)
}

/// Borrowed object serialization with canonical entry ordering.
struct CanonicalObject<'a>(&'a Map<String, Value>);

impl Serialize for CanonicalObject<'_> {
    /// Serialize object entries without owning a copy of the arguments.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in entries(self.0) {
            map.serialize_entry(key, &Canonical(value))?;
        }
        map.end()
    }
}

/// Serialization view applying canonical policies recursively.
struct Canonical<'a>(&'a Value);

impl Serialize for Canonical<'_> {
    /// Serialize nested values through the canonical key and number policies.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Value::Object(object) => CanonicalObject(object).serialize(serializer),
            Value::Array(array) => {
                let mut sequence = serializer.serialize_seq(Some(array.len()))?;
                for value in array {
                    sequence.serialize_element(&Canonical(value))?;
                }
                sequence.end()
            }
            Value::Number(number) => match number.as_f64() {
                Some(number) => serializer.serialize_f64(number),
                None => number.serialize(serializer),
            },
            value => value.serialize(serializer),
        }
    }
}

/// Pretty JSON formatter overriding only binary64 number spelling.
struct NumberFormatter<'a>(PrettyFormatter<'a>);

/// Delegate unchanged structural formatting to the existing pretty formatter.
macro_rules! forward {
    ($($method:ident),* $(,)?) => { $(
        fn $method<W: Write + ?Sized>(&mut self, writer: &mut W) -> io::Result<()> {
            self.0.$method(writer)
        }
    )* };
}

impl Formatter for NumberFormatter<'_> {
    forward!(
        begin_array,
        end_array,
        end_array_value,
        begin_object,
        end_object,
        begin_object_value,
        end_object_value
    );

    /// Preserve the pretty formatter's array indentation and delimiters.
    fn begin_array_value<W: Write + ?Sized>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.0.begin_array_value(writer, first)
    }

    /// Preserve the pretty formatter's object indentation and delimiters.
    fn begin_object_key<W: Write + ?Sized>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.0.begin_object_key(writer, first)
    }

    /// Write shortest binary64 spelling with the established exponent thresholds.
    fn write_f64<W: Write + ?Sized>(&mut self, writer: &mut W, value: f64) -> io::Result<()> {
        writer.write_all(ryu_js::Buffer::new().format(value).as_bytes())
    }
}
