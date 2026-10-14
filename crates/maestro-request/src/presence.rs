//! The representation of an optional property that keeps omission apart from null.

use serde::de::{Deserialize, Deserializer};
use serde::ser::{Error as _, Serialize, Serializer};

/// An optional property of a record: omitted, explicitly null, or a value.
///
/// Record fields may skip [`Missing`](Presence::Missing) during serialization and default
/// to it when absent. Null is distinct from omission.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Presence<T> {
    /// The property is absent.
    #[default]
    Missing,
    /// The property is present and null.
    Null,
    /// The property is present with a value.
    Present(T),
}

impl<T> Presence<T> {
    /// Whether the property is absent; the predicate that decides which fields serialization
    /// skips.
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }
}

impl<T: Serialize> Serialize for Presence<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Missing => Err(S::Error::custom("missing value outside a property")),
            Self::Null => serializer.serialize_none(),
            Self::Present(value) => value.serialize(serializer),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Presence<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Option::<T>::deserialize(deserializer)?.map_or(Self::Null, Self::Present))
    }
}

/// A record field type that serialization may leave out.
pub trait Omittable {
    /// Whether serialization leaves the field out; never, unless the type says otherwise.
    fn is_omitted(&self) -> bool {
        false
    }
}
impl Omittable for String {}
impl<T> Omittable for Presence<T> {
    fn is_omitted(&self) -> bool {
        self.is_missing()
    }
}
impl<T> Omittable for Option<T> {
    fn is_omitted(&self) -> bool {
        self.is_none()
    }
}
