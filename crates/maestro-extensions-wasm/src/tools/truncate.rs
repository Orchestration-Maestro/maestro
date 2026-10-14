//! Supplied truncate tool data.
/// The limit reported as causing truncation.
pub use maestro_tool::tools::truncate::TruncationBy as TruncationLimit;
/// Supplied truncation output with floating-point accounting.
pub type TruncationResult = maestro_tool::tools::truncate::TruncationResult<f64>;
