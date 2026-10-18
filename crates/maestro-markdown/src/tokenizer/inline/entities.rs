//! Native entity decoding and punctuation unescaping.
use regress::Regex;
use std::{collections::HashMap, ops::Range, sync::LazyLock};

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

/// Decode a structured operand once, before it reaches a renderer.
pub(crate) fn decode_operand(source: &str) -> String {
    decode_with(source, OPERANDS.as_ref())
}

/// Share entity substitution without sharing the callers' escape alphabets.
fn decode_with(source: &str, units: Option<&Regex>) -> String {
    let mut output = String::new();
    visit_units(source, units, |_, text| output.push_str(text));
    output
}

/// Retain each native transformation's authored extent for mapped text consumers.
pub(super) fn decode_units(source: &str, emit: impl FnMut(Range<usize>, &str)) {
    visit_units(source, UNITS.as_ref(), emit);
}

/// Decode once, emitting unchanged spans and individual atomic transformations.
fn visit_units(source: &str, units: Option<&Regex>, mut emit: impl FnMut(Range<usize>, &str)) {
    let Some(units) = units else {
        emit(0..source.len(), source);
        return;
    };
    let mut cursor = 0;
    for matched in units.find_iter(source) {
        if cursor < matched.start() {
            emit(cursor..matched.start(), &source[cursor..matched.start()]);
        }
        let unit = &source[matched.range()];
        if let Some(escaped) = unit.strip_prefix('\\') {
            emit(matched.range(), escaped);
        } else if let Some(number) = unit
            .strip_prefix("&#")
            .and_then(|text| text.strip_suffix(';'))
        {
            let value = number.strip_prefix(['x', 'X']).map_or_else(
                || number.parse::<u32>(),
                |digits| u32::from_str_radix(digits, 16),
            );
            let scalar = value
                .ok()
                .filter(|value| *value != 0)
                .and_then(char::from_u32)
                .unwrap_or('\u{fffd}');
            emit(matched.range(), scalar.encode_utf8(&mut [0; 4]));
        } else {
            emit(matched.range(), NAMED.get(unit).copied().unwrap_or(unit));
        }
        cursor = matched.end();
    }
    if cursor < source.len() {
        emit(cursor..source.len(), &source[cursor..]);
    }
}
