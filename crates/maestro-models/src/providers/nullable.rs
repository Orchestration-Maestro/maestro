//! Nullable field values kept separate from omission.

use serde::{Deserialize, Serialize};

/// The value domain of a nullable field, separate from its presence.
#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub(crate) enum Nullable<T> {
    /// Explicit JSON null.
    Null,
    /// Typed field value.
    Value(T),
}
