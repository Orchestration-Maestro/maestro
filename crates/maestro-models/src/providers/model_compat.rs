//! Reads of a model's compatibility flags shared by the protocol adapters.

use maestro_json::is_truthy;

use crate::Model;

/// The model's compatibility flag `name`: `default` when the model leaves it out or sets it to
/// `null`, otherwise whether the value is truthy.
pub(crate) fn compat_flag(model: &Model, name: &str, default: bool) -> bool {
    model
        .compat
        .as_ref()
        .and_then(|compat| compat.0.get(name))
        .filter(|value| !value.is_null())
        .map_or(default, is_truthy)
}
