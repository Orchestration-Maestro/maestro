//! JSON text as ECMAScript `JSON.stringify` writes it, and the reading of numbers and objects
//! that `JSON.parse` gives: every number is the double it rounds to.

pub(crate) use crate::parameters::parsed_arguments;

pub use maestro_json::compact_json;
pub(crate) use maestro_json::is_raw_truthy as is_truthy;
pub(crate) use maestro_json::raw_value::{
    EntryKey, MAX_NESTING, compact_raw, json_value, lenient, member, number, object_record,
    or_zero, raw_json, raw_number, record, required, try_object_record,
};
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use maestro_json::{BorrowedJson, array_index, compact_members};

#[cfg(test)]
mod tests;
