//! Metadata observations retain kinds, key order and exceptional floating-point values.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionAPI, ExtensionResult, frontmatter::FrontmatterValue};
use serde_json::{Value, json};
/// Reads metadata without serializing a nonfinite number as null.
pub(super) fn observe(api: &ExtensionAPI, request: &Value) -> ExtensionResult<Value> {
    let text = request["args"][0].as_str().ok_or("missing content")?;
    let result = (|| -> ExtensionResult<Value> {
        match request["operation"].as_str() {
            Some("fm.parseFrontmatter") => {
                let parsed = maestro_extensions_wasm::frontmatter::parse_frontmatter(api, text)?;
                Ok(json!({"frontmatter":value(parsed.frontmatter),"body":parsed.body}))
            }
            Some("fm.parseYaml") => Ok(value(maestro_extensions_wasm::frontmatter::parse_yaml(
                api, text,
            )?)),
            Some("fm.stripFrontmatter") => Ok(json!(
                maestro_extensions_wasm::frontmatter::strip_frontmatter(api, text)?
            )),
            _ => Err("unknown helper observation".into()),
        }
    })();
    match result {
        Ok(value) => Ok(value),
        Err(error) => Ok(json!({"error":error})),
    }
}
/// Projects the typed result for observation only; no production number encoding uses this.
fn value(input: FrontmatterValue) -> Value {
    match input {
        FrontmatterValue::Null => json!({"kind":"null"}),
        FrontmatterValue::Bool(value) => json!({"kind":"bool","value":value}),
        FrontmatterValue::Number(number) => {
            let value = if number.is_nan() {
                json!({"$number":"NaN"})
            } else if number.is_infinite() {
                json!({"$number":if number.is_sign_negative() {"-Infinity"} else {"Infinity"}})
            } else if number == 0.0 && number.is_sign_negative() {
                json!({"$number":"-0"})
            } else {
                json!(number)
            };
            json!({"kind":"number","value":value})
        }
        FrontmatterValue::String(value) => json!({"kind":"string","value":value}),
        FrontmatterValue::Sequence(items) => {
            json!({"kind":"sequence","value":items.into_iter().map(value).collect::<Vec<_>>()})
        }
        FrontmatterValue::Mapping(entries) => {
            json!({"kind":"mapping","value":entries.into_iter().map(|(key,item)|(key,value(item))).collect::<Vec<_>>()})
        }
    }
}
