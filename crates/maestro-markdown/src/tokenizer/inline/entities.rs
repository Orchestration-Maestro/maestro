//! Native entity decoding and punctuation unescaping.
use regress::Regex;
use std::{collections::HashMap, sync::LazyLock};

/// Complete approved named-entity table, shared across parser invocations.
static NAMED: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    entities::ENTITIES
        .iter()
        .map(|entry| (entry.entity, entry.characters))
        .collect()
});
/// Native entity syntax is shared by text and structured operand fields.
const ENTITY: &str = r"&(?:#[0-9]{1,7}|#[xX][0-9a-fA-F]{1,6}|[A-Za-z][A-Za-z0-9]{1,31});";
/// Ordinary text admits ASCII punctuation escapes.
static UNITS: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(&format!(
        r"\\[\x21-\x2F\x3A-\x40\x5B-\x60\x7B-\x7E]|{ENTITY}"
    ))
    .ok()
});
/// Destinations, titles and information fields admit Unicode punctuation escapes.
static OPERANDS: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::with_flags(&format!(r"\\[\p{{P}}\p{{S}}]|{ENTITY}"), "u").ok());

/// Decode one authored native field, preserving malformed and semicolonless units.
pub(crate) fn decode(source: &str) -> String {
    decode_with(source, UNITS.as_ref())
}

/// Decode a structured operand once, before it reaches a renderer.
pub(crate) fn decode_operand(source: &str) -> String {
    decode_with(source, OPERANDS.as_ref())
}

/// Share entity substitution without sharing the callers' escape alphabets.
fn decode_with(source: &str, units: Option<&Regex>) -> String {
    let Some(units) = units else {
        return source.to_owned();
    };
    let mut output = String::new();
    let mut cursor = 0;
    for matched in units.find_iter(source) {
        output.push_str(&source[cursor..matched.start()]);
        let unit = &source[matched.range()];
        if let Some(escaped) = unit.strip_prefix('\\') {
            output.push_str(escaped);
        } else if let Some(number) = unit
            .strip_prefix("&#")
            .and_then(|text| text.strip_suffix(';'))
        {
            let value = number.strip_prefix(['x', 'X']).map_or_else(
                || number.parse::<u32>(),
                |digits| u32::from_str_radix(digits, 16),
            );
            output.push(
                value
                    .ok()
                    .filter(|value| *value != 0)
                    .and_then(char::from_u32)
                    .unwrap_or('\u{fffd}'),
            );
        } else {
            output.push_str(NAMED.get(unit).copied().unwrap_or(unit));
        }
        cursor = matched.end();
    }
    output.push_str(&source[cursor..]);
    output
}
