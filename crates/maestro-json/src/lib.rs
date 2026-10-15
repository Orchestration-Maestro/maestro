//! Stateless JSON boundary decisions shared by Maestro crates: truth, canonical key
//! order, compact and pretty text, and object-only record admission.
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
mod keys;
pub mod object;
mod truth;

pub use json_text::{
    BorrowedJson, compact_json, compact_members, compact_object, pretty_json, pretty_object,
};
pub use keys::{array_index, canonical_entries};
pub use truth::{is_raw_truthy, is_truthy};
