//! Browser-facing application policies.

mod decimal;
pub mod docx_preview;
mod links;
pub mod ssf;

pub use decimal::strip_decimal;
pub use links::{executable, script_scheme};

/// Attachment source resolution.
pub mod utils;

/// Browser conversation records.
pub mod components;

/// Executable browser tools.
pub mod tools;

/// Injected collection storage.
pub mod persistence;
