//! Canonical JSON text and the model argument parser's raw-value boundary.
pub use maestro_json::compact_json;
pub(crate) use maestro_json::raw_value::{MAX_NESTING, json_value, raw_json};
#[cfg(test)]
pub(crate) use maestro_json::raw_value::{member, object_record, or_zero, raw_number};
#[cfg(test)]
mod tests;
