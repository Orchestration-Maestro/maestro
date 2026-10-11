//! Output helpers for Maestro tools.
//!
//! ```
//! use maestro_tools::{TruncationOptions, truncate_head};
//!
//! let selected = truncate_head("first\nlast", TruncationOptions {
//!     max_lines: Some(1),
//!     ..TruncationOptions::default()
//! });
//! assert_eq!(selected.content, "first");
//! assert_eq!(selected.total_lines, 2);
//! ```

pub mod output;

pub use output::truncate::{
    DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES, TruncateLineResult, TruncationBy, TruncationOptions,
    TruncationResult, format_size, truncate_head, truncate_line, truncate_tail,
};
