//! Text primitives shared by Maestro crates: streaming and BOM-aware lossy UTF-8,
//! whitespace membership, UTF-16 lexical order, markup escaping, CSS string serialization,
//! environment flag truth,
//! optional `collation` locale comparison and optional `number-format` locale-number presentation.
//!
//! ```
//! use maestro_text::{is_whitespace_scalar, markup::{EscapeMode, escape}, utf16_order};
//! use std::cmp::Ordering;
//!
//! assert!(is_whitespace_scalar('\u{feff}') && !is_whitespace_scalar('\u{85}'));
//! assert_eq!(utf16_order("\u{1f600}", "\u{e000}"), Ordering::Less);
//! assert_eq!(escape("'&", EscapeMode::Xml), "&apos;&amp;");
//! assert_eq!(escape("'", EscapeMode::HtmlDecimal).len(), 5);
//! assert_eq!(escape("'", EscapeMode::HtmlHex).len(), 6);
//! ```
#[cfg(feature = "collation")]
pub mod collation;
pub mod css;
mod flags;
mod lexical;
pub mod markup;
mod newlines;
pub mod terminal;
pub mod utf8;
mod whitespace;
pub use utf8::{Utf8Stream, decode_utf8, lossy_string};

pub use flags::{is_offline_selected, is_truthy_env_flag};
pub use lexical::{truncate_utf16, utf16_order};
pub use newlines::normalize_to_lf;
pub use whitespace::{WHITESPACE, is_whitespace_scalar};

#[cfg(any(feature = "collation", feature = "number-format"))]
mod locale;
#[cfg(feature = "number-format")]
pub mod number_format;
