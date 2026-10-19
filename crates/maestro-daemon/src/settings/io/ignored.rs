//! JSON deserialization that records paths consumed as ignored values.

use std::{cell::RefCell, slice};

use serde::de::{DeserializeSeed, IntoDeserializer, MapAccess, SeqAccess, Visitor};
use serde_json::Value;

/// Sortable key/index identity, kept unescaped until diagnostic formatting.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Segment {
    /// Authored object member.
    Key(String),
    /// Array position.
    Index(usize),
}

/// Borrowed raw value or selected object fields from independent namespaces.
pub(super) enum ValueRef<'a> {
    /// Unmodified authored JSON subtree.
    Value(&'a Value),
    /// Shallow selection retaining borrowed keys and values.
    Fields(Vec<(&'a str, &'a Value)>),
}

/// A borrowed JSON subtree with its diagnostic path and shared ignored paths.
pub(super) struct Decoder<'a> {
    /// Current JSON value.
    pub value: ValueRef<'a>,
    /// Stored key path, including its namespace.
    pub path: Vec<Segment>,
    /// Paths ignored by the typed decoder.
    pub ignored: &'a RefCell<Vec<Vec<Segment>>>,
}

impl<'de> serde::Deserializer<'de> for Decoder<'de> {
    type Error = serde_json::Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.value {
            ValueRef::Fields(values) => visitor.visit_map(Members {
                iter: values.into_iter(),
                pending: None,
                path: self.path,
                ignored: self.ignored,
            }),
            ValueRef::Value(Value::Object(values)) => visitor.visit_map(Members {
                iter: values.iter().map(|(key, value)| (key.as_str(), value)),
                pending: None,
                path: self.path,
                ignored: self.ignored,
            }),
            ValueRef::Value(Value::Array(values)) => visitor.visit_seq(Elements {
                iter: values.iter(),
                index: 0,
                parent: self,
            }),
            ValueRef::Value(value) => serde::Deserializer::deserialize_any(value, visitor),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        if matches!(self.value, ValueRef::Value(Value::Null)) {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        let ValueRef::Value(value @ Value::String(_)) = self.value else {
            return Err(serde::de::Error::custom("settings enum must be a string"));
        };
        serde::Deserializer::deserialize_enum(value, name, variants, visitor)
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        if matches!(self.value, ValueRef::Value(value) if !value.is_object()) {
            return Err(serde::de::Error::custom(
                "settings record must be an object",
            ));
        }
        self.deserialize_any(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.ignored.borrow_mut().push(self.path);
        visitor.visit_unit()
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct seq tuple tuple_struct map identifier
    }
}

/// JSON object traversal preserving typed field ownership.
struct Members<'a, I> {
    /// Remaining members, visited once.
    iter: I,
    /// Value associated with the most recently decoded key.
    pending: Option<(&'a str, &'a Value)>,
    /// Parent diagnostic path.
    path: Vec<Segment>,
    /// Shared ignored paths.
    ignored: &'a RefCell<Vec<Vec<Segment>>>,
}

impl<'de, I: ExactSizeIterator<Item = (&'de str, &'de Value)>> MapAccess<'de> for Members<'de, I> {
    type Error = serde_json::Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Self::Error> {
        let Some((key, value)) = self.iter.next() else {
            return Ok(None);
        };
        self.pending = Some((key, value));
        seed.deserialize(key.into_deserializer()).map(Some)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, Self::Error> {
        let (key, value) = self
            .pending
            .take()
            .ok_or_else(|| serde::de::Error::custom("settings value requested without its key"))?;
        let mut path = self.path.clone();
        path.push(Segment::Key(key.to_owned()));
        seed.deserialize(Decoder {
            value: ValueRef::Value(value),
            path,
            ignored: self.ignored,
        })
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

/// JSON array traversal preserving each element's diagnostic index.
struct Elements<'a> {
    /// Remaining elements, visited once.
    iter: slice::Iter<'a, Value>,
    /// Next element's index.
    index: usize,
    /// Parent path and diagnostic collection.
    parent: Decoder<'a>,
}

impl<'de> SeqAccess<'de> for Elements<'de> {
    type Error = serde_json::Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Self::Error> {
        let Some(value) = self.iter.next() else {
            return Ok(None);
        };
        let mut path = self.parent.path.clone();
        path.push(Segment::Index(self.index));
        self.index += 1;
        seed.deserialize(Decoder {
            value: ValueRef::Value(value),
            path,
            ignored: self.parent.ignored,
        })
        .map(Some)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

/// Quote keys whose punctuation would otherwise resemble path separators.
pub(super) fn path_key(key: &str) -> String {
    if !key.is_empty()
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        key.to_owned()
    } else {
        Value::String(key.to_owned()).to_string()
    }
}

/// Format a source-ordered path without confusing dotted keys and indices.
pub(super) fn format_path(path: &[Segment]) -> String {
    path.iter()
        .map(|segment| match segment {
            Segment::Key(key) => path_key(key),
            Segment::Index(index) => index.to_string(),
        })
        .collect::<Vec<_>>()
        .join(".")
}
