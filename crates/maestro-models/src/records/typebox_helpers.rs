//! String-enum schemas without alternative constant branches.

use serde_json::{Value, json};

/// Optional strings copied into a string-enum schema when nonempty.
#[derive(Clone, Copy, Debug, Default)]
pub struct StringEnumOptions<'a> {
    /// Description of the accepted values.
    pub description: Option<&'a str>,
    /// Supplied default, without membership validation.
    pub default: Option<&'a str>,
}

/// Build a string schema retaining value order and nonempty supplied options.
#[must_use]
pub fn string_enum(values: &[&str], options: Option<StringEnumOptions<'_>>) -> Value {
    let mut schema = json!({"type": "string", "enum": values});
    if let Some(options) = options {
        for (key, value) in [
            ("description", options.description),
            ("default", options.default),
        ] {
            if let Some(value) = value.filter(|value| !value.is_empty()) {
                schema[key] = json!(value);
            }
        }
    }
    schema
}
