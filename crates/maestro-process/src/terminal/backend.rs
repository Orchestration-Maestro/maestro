//! Native terminal creation and setup entry.
mod unix;
pub use unix::{setup_and_exec, spawn};
