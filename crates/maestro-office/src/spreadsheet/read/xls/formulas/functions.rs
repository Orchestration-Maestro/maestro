//! One selector for both immutable function descriptor tables.
use super::{cetab::CETAB, ftab::FTAB, types::FunctionId};
/// Identifier, spelling and optional fixed arity.
pub(super) type Entry = (u16, &'static str, Option<u8>);
/// Find a descriptor without rebuilding or duplicating lookup logic.
#[must_use]
pub(super) fn function(function: FunctionId) -> Option<(&'static str, Option<u8>)> {
    let (table, id) = match function {
        FunctionId::Worksheet(id) => (FTAB, id),
        FunctionId::Command(id) => (CETAB, id),
    };
    table
        .binary_search_by_key(&id, |entry| entry.0)
        .ok()
        .map(|index| (table[index].1, table[index].2))
}
