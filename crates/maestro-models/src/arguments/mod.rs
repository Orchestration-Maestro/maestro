#![doc = include_str!("../../../../docs/models/arguments.md")]
pub mod sanitize_unicode;
pub use sanitize_unicode::sanitize_surrogates;
pub mod json_parse;
pub use json_parse::{parse_json_with_repair, parse_streaming_json, repair_json};

pub mod validation;
pub use validation::{validate_tool_arguments, validate_tool_call};
