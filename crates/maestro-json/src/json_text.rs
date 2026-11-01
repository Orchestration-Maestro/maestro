//! Canonical JSON text with double spelling or exact native integers.

use std::{
    borrow::Cow,
    io::{self, Write},
};

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
    write::<false>(&Canonical(value), CompactFormatter)
}

/// Display a selected truthy value without changing strings.
///
/// Absent and falsy values have no display; other values use compact JSON.
/// # Errors
/// Returns the compact writer's serialization failure.
pub fn display_truthy(value: Option<&Value>) -> Result<Option<Cow<'_, str>>, serde_json::Error> {
    let Some(value) = value.filter(|value| crate::is_truthy(value)) else {
        return Ok(None);
    };
    if let Some(text) = value.as_str() {
        return Ok(Some(Cow::Borrowed(text)));
    }
    compact_json(value).map(|text| Some(Cow::Owned(text)))
}

/// Write an object exactly as [`compact_json`] writes a value holding it.
///
/// # Errors
/// Returns the serializer failure.
pub fn compact_object(value: &Map<String, Value>) -> Result<String, serde_json::Error> {
    write::<false>(&Members(object_entries(value)), CompactFormatter)
}

/// Write borrowed members in the order and spelling [`compact_object`] gives an object, but
/// write every member given: a repeated name is written once per occurrence.
///
/// # Errors
/// Returns the serializer failure.
pub fn compact_members<'a>(
    members: impl Iterator<Item = (&'a str, BorrowedJson<'a>)>,
) -> Result<String, serde_json::Error> {
    write::<false>(&Members(canonical_entries(members)), CompactFormatter)
}

/// Write borrowed members with the named member first, using its authored value or the fallback.
///
/// The first matching member supplies the value; its other occurrences are omitted.
/// Canonical ordering and number spelling follow [`compact_members`], so index keys still lead.
///
/// # Errors
/// Returns the serializer failure.
pub fn compact_members_with_default<'a>(
    members: impl Iterator<Item = (&'a str, BorrowedJson<'a>)> + Clone,
    name: &'a str,
    fallback: BorrowedJson<'a>,
) -> Result<String, serde_json::Error> {
    let value = members
        .clone()
        .find_map(|(key, value)| (key == name).then_some(value))
        .unwrap_or(fallback);
    compact_members(std::iter::once((name, value)).chain(members.filter(|(key, _)| *key != name)))
}

/// Write an object with two-space indentation and no final newline, in the order and
/// number spelling of [`compact_json`].
///
/// # Errors
/// Returns the serializer failure.
pub fn pretty_object(value: &Map<String, Value>) -> Result<String, serde_json::Error> {
    write::<false>(
        &Members(object_entries(value)),
        PrettyFormatter::with_indent(b"  "),
    )
}

/// Write any JSON value with two-space indentation and canonical keys and numbers.
///
/// # Errors
/// Returns the serializer failure.
pub fn pretty_json(value: &Value) -> Result<String, serde_json::Error> {
    write::<false>(&Canonical(value), PrettyFormatter::with_indent(b"  "))
}

/// Write canonical keys and floating-point spelling while retaining native integers.
pub(crate) fn compact_native_json(value: &Value) -> Result<String, serde_json::Error> {
    write::<true>(&Canonical(value), CompactFormatter)
}

/// Serialize with the number spelling layered over the given structural formatter.
fn write<const PRESERVE_INTEGERS: bool>(
    value: &impl Serialize,
    formatter: impl Formatter,
) -> Result<String, serde_json::Error> {
    let mut serializer = serde_json::Serializer::with_formatter(
        Vec::new(),
        Numbers::<_, PRESERVE_INTEGERS>(formatter),
    );
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
    /// Order object keys and leave scalar encoding to the serializer.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Value::Object(object) => Members(object_entries(object)).serialize(serializer),
            Value::Array(items) => BorrowedJson::Slice(items).serialize(serializer),
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
struct Numbers<F, const PRESERVE_INTEGERS: bool>(F);

impl<F: Formatter, const PRESERVE_INTEGERS: bool> Numbers<F, PRESERVE_INTEGERS> {
    /// Keep native integer spelling or apply the canonical double conversion.
    fn write_integer<W: Write + ?Sized>(
        &mut self,
        writer: &mut W,
        number: &serde_json::Number,
    ) -> io::Result<()> {
        if !PRESERVE_INTEGERS && let Some(double) = number.as_f64() {
            self.write_f64(writer, double)
        } else {
            write!(writer, "{number}")
        }
    }
}

impl<F: Formatter, const PRESERVE_INTEGERS: bool> Formatter for Numbers<F, PRESERVE_INTEGERS> {
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

    /// Write a signed integer using the selected numeric mode.
    fn write_i64<W: Write + ?Sized>(&mut self, writer: &mut W, value: i64) -> io::Result<()> {
        self.write_integer(writer, &value.into())
    }

    /// Write an unsigned integer using the selected numeric mode.
    fn write_u64<W: Write + ?Sized>(&mut self, writer: &mut W, value: u64) -> io::Result<()> {
        self.write_integer(writer, &value.into())
    }

    /// Write the shortest spelling that ECMAScript prints for the double.
    fn write_f64<W: Write + ?Sized>(&mut self, writer: &mut W, value: f64) -> io::Result<()> {
        writer.write_all(ryu_js::Buffer::new().format_finite(value).as_bytes())
    }
}
