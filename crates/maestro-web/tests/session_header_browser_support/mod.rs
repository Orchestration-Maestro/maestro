//! Header disclosure witnesses using the shared native slot harness.
pub(crate) use crate::reading_slots_support::{Slots, js, snapshot, unchanged};
mod disclosures;
/// Native faults below the reading placement and disclosure registration.
pub(crate) mod hooks;
