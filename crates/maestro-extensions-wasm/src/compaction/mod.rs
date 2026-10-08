//! Compaction records.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

mod utils;

pub use crate::bindings::maestro::extension::session::{
    CompactionPreparation, CompactionResult, CompactionSettings,
};
pub use utils::FileOperations;
