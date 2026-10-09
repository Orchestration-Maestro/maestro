//! Records read from JSON objects only.
//!
//! A derived record also accepts a JSON array and reads its elements as the properties in
//! declaration order. The documents an extension receives name their properties, so the event
//! and its nested records are read from the map of a JSON object here, and a positional array is
//! refused.

use std::fmt;
use std::marker::PhantomData;

use serde::de::value::MapAccessDeserializer;
use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};

use super::presence::Presence;

/// A record read from a JSON object.
struct Object<T>(T);

/// Hands the map of a JSON object to the record `T`.
struct ObjectVisitor<T>(PhantomData<T>);

impl<'de, T: Deserialize<'de>> Visitor<'de> for ObjectVisitor<T> {
    type Value = Object<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON object")
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
        T::deserialize(MapAccessDeserializer::new(map)).map(Object)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Object<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(ObjectVisitor(PhantomData))
    }
}

/// Reads the record `T` from the JSON object in `text`.
///
/// # Errors
/// Returns an error for malformed text, for a document that is not an object, and for an
/// object `T` does not accept.
#[cfg(any(test, target_arch = "wasm32"))]
pub(crate) fn from_str<'de, T: Deserialize<'de>>(text: &'de str) -> Result<T, serde_json::Error> {
    serde_json::from_str::<Object<T>>(text).map(|Object(record)| record)
}

/// Reads the record `T` from a property whose value must be a JSON object.
///
/// # Errors
/// Returns the deserializer's error for any other value, and for an object `T` does not accept.
pub(crate) fn record<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<T, D::Error> {
    Object::deserialize(deserializer).map(|Object(record)| record)
}

/// Reads an optional list of records from a property that is null or a list of JSON objects.
///
/// # Errors
/// Returns the deserializer's error for any other value, for an element that is not an object,
/// and for an object `T` does not accept.
pub(crate) fn records<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Presence<Vec<T>>, D::Error> {
    let list = Option::<Vec<Object<T>>>::deserialize(deserializer)?;
    Ok(list.map_or(Presence::Null, |records| {
        Presence::Present(records.into_iter().map(|Object(record)| record).collect())
    }))
}
