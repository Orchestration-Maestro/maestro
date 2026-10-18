//! Records read from JSON objects only, by the readers of [`maestro_json::object`].
//!
//! Only the optional list is adapted here, to the shared `Presence` record.

use serde::de::{Deserialize, Deserializer};

use crate::presence::Presence;

pub use maestro_json::object::{list, record};

/// Reads an optional list of records from a property that is null or a list of JSON objects.
///
/// # Errors
/// Returns the deserializer's error for any other value, for an element that is not an object,
/// and for an object `T` does not accept.
pub fn records<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Presence<Vec<T>>, D::Error> {
    maestro_json::object::optional_list(deserializer)
        .map(|list| list.map_or(Presence::Null, Presence::Present))
}
