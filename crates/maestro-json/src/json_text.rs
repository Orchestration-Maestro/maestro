//! Canonical JSON text: object keys in canonical order and every number as the double it
//! rounds to, in shortest ECMAScript spelling.

use std::io::{self, Write};

use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Serialize, Serializer};
use serde_json::ser::{CompactFormatter, Formatter, PrettyFormatter};
use serde_json::{Map, Value};

use crate::keys::canonical_entries;

/// A borrowed value or array slice written without allocating a JSON container.
#[derive(Clone, Copy)]
pub enum BorrowedJson<'a> {
    /// One complete value.
    Value(&'a Value),
    /// Array elements.
    Slice(&'a [Value]),
}

/// Write compact canonical JSON text.
///
/// Keys that are canonical array indices come first in ascending numeric order, then every
/// other key in insertion order. A number is written as the double it names, so an integer
/// beyond 2^53 is rounded and negative zero is `0`.
///
/// # Errors
/// Returns the serializer failure.
pub fn compact_json(value: &Value) -> Result<String, serde_json::Error> {
    write(&Canonical(value), CompactFormatter)
}

/// Write an object exactly as [`compact_json`] writes a value holding it.
///
/// # Errors
/// Returns the serializer failure.
pub fn compact_object(value: &Map<String, Value>) -> Result<String, serde_json::Error> {
    write(&Members(object_entries(value)), CompactFormatter)
}

/// Write borrowed members in the order and spelling [`compact_object`] gives an object, but
/// write every member given: a repeated name is written once per occurrence.
///
/// # Errors
/// Returns the serializer failure.
pub fn compact_members<'a>(
    members: impl Iterator<Item = (&'a str, BorrowedJson<'a>)>,
) -> Result<String, serde_json::Error> {
    write(&Members(canonical_entries(members)), CompactFormatter)
}

/// Write an object with two-space indentation and no final newline, in the order and
/// number spelling of [`compact_json`].
///
/// # Errors
/// Returns the serializer failure.
pub fn pretty_object(value: &Map<String, Value>) -> Result<String, serde_json::Error> {
    write(
        &Members(object_entries(value)),
        PrettyFormatter::with_indent(b"  "),
    )
}

/// Write any JSON value with two-space indentation and canonical keys and numbers.
///
/// # Errors
/// Returns the serializer failure.
pub fn pretty_json(value: &Value) -> Result<String, serde_json::Error> {
    write(&Canonical(value), PrettyFormatter::with_indent(b"  "))
}

/// Serialize with the number spelling layered over the given structural formatter.
fn write(value: &impl Serialize, formatter: impl Formatter) -> Result<String, serde_json::Error> {
    let mut serializer = serde_json::Serializer::with_formatter(Vec::new(), Numbers(formatter));
    value.serialize(&mut serializer)?;
    String::from_utf8(serializer.into_inner())
        .map_err(|error| serde_json::Error::io(io::Error::new(io::ErrorKind::InvalidData, error)))
}

/// Borrow an object's entries as members.
fn object_entries(object: &Map<String, Value>) -> Vec<(&str, BorrowedJson<'_>)> {
    canonical_entries(
        object
            .iter()
            .map(|(key, value)| (key.as_str(), BorrowedJson::Value(value))),
    )
}

/// Members already in canonical order.
struct Members<'a>(Vec<(&'a str, BorrowedJson<'a>)>);

impl Serialize for Members<'_> {
    /// Write each member in the order held.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

impl Serialize for BorrowedJson<'_> {
    /// Write a value or the elements of a slice through the canonical view.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Value(value) => Canonical(value).serialize(serializer),
            Self::Slice(items) => {
                let mut sequence = serializer.serialize_seq(Some(items.len()))?;
                for item in *items {
                    sequence.serialize_element(&Canonical(item))?;
                }
                sequence.end()
            }
        }
    }
}

/// A value serialized with canonical key order at every depth.
struct Canonical<'a>(&'a Value);

impl Serialize for Canonical<'_> {
    /// Order object keys, spell numbers as doubles and leave other values to serde.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Value::Object(object) => Members(object_entries(object)).serialize(serializer),
            Value::Array(items) => BorrowedJson::Slice(items).serialize(serializer),
            Value::Number(number) => match number.as_f64() {
                Some(double) => serializer.serialize_f64(double),
                None => number.serialize(serializer),
            },
            value => value.serialize(serializer),
        }
    }
}

/// Delegate structural formatting unchanged.
macro_rules! forward {
    ($($method:ident),* $(,)?) => { $(
        fn $method<W: Write + ?Sized>(&mut self, writer: &mut W) -> io::Result<()> {
            self.0.$method(writer)
        }
    )* };
}

/// A formatter that spells doubles as ECMAScript does and leaves layout to the inner one.
struct Numbers<F>(F);

impl<F: Formatter> Formatter for Numbers<F> {
    forward!(
        begin_array,
        end_array,
        end_array_value,
        begin_object,
        end_object,
        begin_object_value,
        end_object_value
    );

    /// Keep the inner element separator and indentation.
    fn begin_array_value<W: Write + ?Sized>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.0.begin_array_value(writer, first)
    }

    /// Keep the inner member separator and indentation.
    fn begin_object_key<W: Write + ?Sized>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.0.begin_object_key(writer, first)
    }

    /// Write the shortest spelling that ECMAScript prints for the double.
    fn write_f64<W: Write + ?Sized>(&mut self, writer: &mut W, value: f64) -> io::Result<()> {
        writer.write_all(ryu_js::Buffer::new().format_finite(value).as_bytes())
    }
}
