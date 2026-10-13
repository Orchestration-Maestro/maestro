//! Stateless text decisions shared by Maestro crates: whitespace membership, UTF-16
//! lexical order, markup escaping and, with the `collation` feature, locale comparison.
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
mod lexical;
pub mod markup;
mod newlines;
mod whitespace;

pub use lexical::utf16_order;
pub use newlines::normalize_to_lf;
pub use whitespace::{WHITESPACE, is_whitespace_scalar};
