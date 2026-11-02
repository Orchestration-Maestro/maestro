//! Records read from JSON objects only.
//!
//! A derived record can also accept a JSON array and read its elements as the properties in
//! declaration order. These readers hand a record only the map of a JSON object and so
//! reject arrays. Defaults, unknown members and nested values stay with the record's own
//! decoders and the caller's deserializer. A repeated member fails the record's own decoder,
//! and `list_naming` refuses one before it decodes.

use std::fmt;
use std::marker::PhantomData;

use serde::Deserialize;
use serde::de::value::{MapAccessDeserializer, MapDeserializer};
use serde::de::{self, DeserializeOwned, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};

/// The expectation reported for a value that is not an object.
const EXPECTING: &str = "a JSON object";

/// A record read from a JSON object.
struct Object<T>(T);

/// Hands the map of a JSON object to the record `T`.
struct ObjectVisitor<'a, T> {
    /// What the caller says it expected, for an invalid-type error.
    expecting: &'a str,
    /// The record type being read.
    record: PhantomData<T>,
}

impl<'de, T: Deserialize<'de>> Visitor<'de> for ObjectVisitor<'_, T> {
    type Value = T;

    /// Name the caller's expectation.
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.expecting)
    }

    /// Read the record from the map.
    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<T, A::Error> {
        T::deserialize(MapAccessDeserializer::new(map))
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Object<T> {
    /// Read the record from a JSON object.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        record(deserializer).map(Object)
    }
}

/// Reads the record `T` from a value that must be a JSON object, naming `expecting` in the
/// error for any other value.
///
/// # Errors
/// Returns the deserializer's error for a value that is not an object, and for an object `T`
/// does not accept.
pub fn record_with<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
    expecting: &str,
) -> Result<T, D::Error> {
    deserializer.deserialize_map(ObjectVisitor {
        expecting,
        record: PhantomData,
    })
}

/// Reads the record `T` from the JSON object in `text`.
///
/// # Errors
/// Returns an error for malformed text, for a document that is not an object, and for an
/// object `T` does not accept.
pub fn from_str<'de, T: Deserialize<'de>>(text: &'de str) -> Result<T, serde_json::Error> {
    serde_json::from_str::<Object<T>>(text).map(|Object(record)| record)
}

/// Reads the record `T` from a property whose value must be a JSON object.
///
/// # Errors
/// Returns the deserializer's error for any other value, and for an object `T` does not accept.
pub fn record<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<T, D::Error> {
    record_with(deserializer, EXPECTING)
}

/// Reads a required list of records, each of which must be a JSON object.
///
/// # Errors
/// Returns the deserializer's error for any other value, for an element that is not an object,
/// and for an object `T` does not accept.
pub fn list<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Vec<T>, D::Error> {
    Vec::<Object<T>>::deserialize(deserializer)
        .map(|records| records.into_iter().map(|Object(record)| record).collect())
}

/// Reads a list of records from a property that is null or a list of JSON objects. A null is
/// `None` and an empty list is `Some` of an empty list; absence stays with the caller.
///
/// # Errors
/// Returns the deserializer's error for any other value, for an element that is not an object,
/// and for an object `T` does not accept.
pub fn optional_list<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<Vec<T>>, D::Error> {
    Option::<Vec<Object<T>>>::deserialize(deserializer).map(|records| {
        records.map(|records| records.into_iter().map(|Object(record)| record).collect())
    })
}

/// Reads a required list of records, leaving out each entry whose string tag `member` the
/// family `T` does not name and decoding every other entry with `T`'s own decoder.
///
/// An entry is left out only where its tag names a variant `T` does not have. A member an
/// object inside the entry repeats, a tag that is not a string, and an entry with no tag at
/// all, fail the whole list, as they do for the record's own decoder.
///
/// # Errors
/// Returns the deserializer's error for a value that is not a list, for an entry that repeats
/// a member of any object inside it, and for a kept entry `T` does not accept.
pub fn list_naming<'de, D: Deserializer<'de>, T: DeserializeOwned>(
    deserializer: D,
    member: &str,
) -> Result<Vec<T>, D::Error> {
    Vec::<Unrepeated>::deserialize(deserializer)?
        .into_iter()
        .filter(|entry| !names_unknown_tag::<T>(&entry.0, member))
        .map(|entry| serde_json::from_value(entry.0).map_err(de::Error::custom))
        .collect()
}

/// Whether `entry` names a tag `member` the family `T` does not name.
fn names_unknown_tag<T: DeserializeOwned>(entry: &Value, member: &str) -> bool {
    entry
        .get(member)
        .and_then(Value::as_str)
        .is_some_and(|kind| !names::<T>(member, kind))
}

/// A JSON value whose every object names each member once.
struct Unrepeated(Value);

impl<'de> Deserialize<'de> for Unrepeated {
    /// Read the value, refusing an object that repeats a member.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(UnrepeatedVisitor)
    }
}

/// Builds a value and refuses an object that names a member twice, at any depth.
struct UnrepeatedVisitor;

impl<'de> Visitor<'de> for UnrepeatedVisitor {
    type Value = Unrepeated;

    /// Name the value being read.
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    /// Read a boolean.
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Unrepeated, E> {
        Ok(Unrepeated(Value::Bool(value)))
    }

    /// Read a signed integer.
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Unrepeated, E> {
        Ok(Unrepeated(Value::from(value)))
    }

    /// Read an unsigned integer.
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Unrepeated, E> {
        Ok(Unrepeated(Value::from(value)))
    }

    /// Read a float.
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Unrepeated, E> {
        serde_json::Number::from_f64(value).map_or_else(
            || Err(de::Error::custom("JSON does not carry a nonfinite number")),
            |number| Ok(Unrepeated(Value::Number(number))),
        )
    }

    /// Read a borrowed string.
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Unrepeated, E> {
        Ok(Unrepeated(Value::String(value.to_owned())))
    }

    /// Read an owned string.
    fn visit_string<E: de::Error>(self, value: String) -> Result<Unrepeated, E> {
        Ok(Unrepeated(Value::String(value)))
    }

    /// Read a null.
    fn visit_unit<E: de::Error>(self) -> Result<Unrepeated, E> {
        Ok(Unrepeated(Value::Null))
    }

    /// Read a null where a value is optional.
    fn visit_none<E: de::Error>(self) -> Result<Unrepeated, E> {
        Ok(Unrepeated(Value::Null))
    }

    /// Read a present value where a value is optional.
    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Unrepeated, D::Error> {
        Unrepeated::deserialize(deserializer)
    }

    /// Read a list.
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Unrepeated, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element::<Unrepeated>()? {
            items.push(item.0);
        }
        Ok(Unrepeated(Value::Array(items)))
    }

    /// Read an object, refusing a member it names twice.
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Unrepeated, A::Error> {
        let mut members = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            let Unrepeated(value) = map.next_value::<Unrepeated>()?;
            if members.insert(key.clone(), value).is_some() {
                return Err(de::Error::custom(format!("repeated member `{key}`")));
            }
        }
        Ok(Unrepeated(Value::Object(members)))
    }
}

/// Whether the family `T`, tagged by the member `member`, names the tag value `kind`.
///
/// The family's own decoder answers: a tag it does not name gives `false`, and a tag it names
/// gives `true` even where the rest of the record is missing or malformed. A `member` the
/// family is not tagged by gives `true`, so a caller that names the wrong member keeps every
/// entry for the family's own decoder to refuse.
#[must_use]
pub fn names<T: DeserializeOwned>(member: &str, kind: &str) -> bool {
    let tag = MapDeserializer::<_, NameProbe>::new(std::iter::once((member, kind)));
    !matches!(T::deserialize(tag), Err(NameProbe::UnknownVariant))
}

/// The outcome of decoding a lone tag: unknown, or any other failure.
#[derive(Debug)]
enum NameProbe {
    /// The decoder reported the tag as an unknown variant.
    UnknownVariant,
    /// Any other failure, such as a missing member of a named variant.
    Other,
}

impl fmt::Display for NameProbe {
    /// Report the outcome for a decoder that only displays it.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnknownVariant => "unknown variant",
            Self::Other => "other failure",
        })
    }
}

impl std::error::Error for NameProbe {}

impl de::Error for NameProbe {
    /// Every other failure keeps its own outcome.
    fn custom<T: fmt::Display>(_message: T) -> Self {
        Self::Other
    }

    /// Record only the unknown-variant outcome as such.
    fn unknown_variant(_variant: &str, _expected: &'static [&'static str]) -> Self {
        Self::UnknownVariant
    }
}
