//! JSON boundary operations shared by Maestro crates: truth, canonical key
//! order, compact and pretty text, object-only record admission and synchronous JSONL framing.
//!
//! ```
//! use maestro_json::{array_index, compact_json, is_truthy};
//! use serde_json::json;
//!
//! assert!(!is_truthy(&json!("")) && is_truthy(&json!([])));
//! assert_eq!(array_index("4294967295"), None);
//! assert_eq!(compact_json(&json!({"b": 1, "10": -0.0, "2": 1e21})).unwrap(), r#"{"2":1e+21,"10":0,"b":1}"#);
//! ```
mod json_text;
pub mod jsonl;
mod keys;
pub mod object;
mod truth;

pub use json_text::{
    BorrowedJson, compact_json, compact_members, compact_members_with_default, compact_object,
    display_truthy, pretty_json, pretty_object,
};
pub use keys::{array_index, canonical_entries};
pub use truth::{is_raw_truthy, is_truthy};

/// Escape one JSON Pointer reference token, leaving ordinary characters unchanged.
#[must_use]
pub fn escape_pointer_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}
