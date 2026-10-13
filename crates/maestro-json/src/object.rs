//! Records read from JSON objects only.
//!
//! A derived record can also accept a JSON array and read its elements as the properties in
//! declaration order. These readers hand a record only the map of a JSON object and so
//! reject arrays. Defaults, duplicate members, unknown members and nested values stay with
//! the record's own decoders and the caller's deserializer.

use std::fmt;
use std::marker::PhantomData;

use serde::de::value::MapAccessDeserializer;
use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};

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
