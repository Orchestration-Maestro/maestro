//! Nullable field values kept separate from omission.

use serde::{Deserialize, Serialize};

/// The value domain of a nullable field, separate from its presence.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Nullable<T> {
    /// Explicit JSON null.
    Null,
    /// Typed field value.
    Value(T),
}
