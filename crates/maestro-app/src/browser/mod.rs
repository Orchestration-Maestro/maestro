//! Browser-facing application policies.

mod decimal;
pub mod ssf;

pub use decimal::strip_decimal;

/// Attachment source resolution.
pub mod utils;

/// Browser conversation records.
pub mod components;
