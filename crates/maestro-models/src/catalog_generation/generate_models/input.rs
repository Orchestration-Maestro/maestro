//! Borrowed feed fields and typed descriptor projection.
use crate::providers::json_text::{EntryKey, is_truthy, member, raw_number};
use crate::{Model, ModelCost, ModelInput};
use serde_json::value::RawValue;

/// Invalid metadata in a selected model.
#[derive(Debug)]
pub(super) struct Invalid;
/// Read a nested optional member without decoding unrelated metadata.
pub(super) fn field<'a>(raw: &'a RawValue, parent: &str, name: &str) -> Option<&'a RawValue> {
    member(raw, parent).and_then(|value| member(value, name))
}
/// Read a required string leaf.
pub(super) fn text(raw: Option<&RawValue>) -> Result<String, Invalid> {
    raw.and_then(|raw| serde_json::from_str(&string_units(raw.get())).ok())
        .ok_or(Invalid)
}
/// Preserve string substring and array element membership, rejecting other non-null kinds.
pub(super) fn includes(raw: Option<&RawValue>, needle: &str) -> Result<bool, Invalid> {
    let Some(raw) = raw.filter(|value| value.get() != "null") else {
        return Ok(false);
    };
    if raw.get().starts_with('"') {
        return Ok(text(Some(raw))?.contains(needle));
    }
    let items: Vec<&RawValue> = serde_json::from_str(raw.get()).map_err(|_| Invalid)?;
    Ok(items
        .into_iter()
        .any(|item| text(Some(item)).is_ok_and(|item| item == needle)))
}
/// Use a default for missing/falsy values; otherwise require a finite number.
pub(super) fn number(raw: Option<&RawValue>, fallback: f64) -> Result<f64, Invalid> {
    let Some(raw) = raw.filter(|raw| is_truthy(raw)) else {
        return Ok(fallback);
    };
    raw_number(raw)
        .filter(|number| number.is_finite())
        .ok_or(Invalid)
}
/// Start a descriptor with its selected route and default metadata.
pub(super) fn descriptor(id: String, api: &str, provider: &str, url: &str) -> Model {
    Model {
        name: id.clone(),
        id,
        api: api.into(),
        provider: provider.into(),
        base_url: url.into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost::default(),
        context_window: 4096.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}
/// Read optional capabilities, prices and limits of a models.dev record.
pub(super) fn dev_metadata(mut model: Model, raw: &RawValue) -> Result<Model, Invalid> {
    model.reasoning = member(raw, "reasoning").is_some_and(|raw| raw.get() == "true");
    if includes(field(raw, "modalities", "input"), "image")? {
        model.input.push(ModelInput::Image);
    }
    model.cost = ModelCost {
        input: number(field(raw, "cost", "input"), 0.0)?,
        output: number(field(raw, "cost", "output"), 0.0)?,
        cache_read: number(field(raw, "cost", "cache_read"), 0.0)?,
        cache_write: number(field(raw, "cost", "cache_write"), 0.0)?,
    };
    model.context_window = number(field(raw, "limit", "context"), model.context_window)?;
    model.max_tokens = number(field(raw, "limit", "output"), model.max_tokens)?;
    Ok(model)
}
/// Decimal prefixes accepted by the external per-token price fields.
static DECIMAL_PREFIX: std::sync::LazyLock<Result<regex::Regex, regex::Error>> =
    std::sync::LazyLock::new(|| {
        regex::Regex::new(r"^[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?")
    });
/// The feed decimal parser's leading whitespace set.
fn price_space(character: char) -> bool {
    matches!(character, '\u{0009}'..='\u{000d}' | ' ' | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}
/// Convert one numeric/string price, replacing nonfinite scaled results with zero.
/// Falsy fields default to zero; the gateway retains the sign of numeric zero.
pub(super) fn price(raw: Option<&RawValue>, preserve_numeric_zero: bool) -> Result<f64, Invalid> {
    let Some(raw) =
        raw.filter(|raw| is_truthy(raw) || (preserve_numeric_zero && raw_number(raw).is_some()))
    else {
        return Ok(0.0);
    };
    let value = if let Some(number) = raw_number(raw) {
        number
    } else {
        let text = text(Some(raw))?;
        DECIMAL_PREFIX
            .as_ref()
            .map_err(|_| Invalid)?
            .find(text.trim_start_matches(price_space))
            .and_then(|prefix| prefix.as_str().parse::<f64>().ok())
            .unwrap_or_default()
    } * 1_000_000.0;
    Ok(if value.is_finite() { value } else { 0.0 })
}
/// Read one complete escaped UTF-16 unit without accepting malformed hex digits.
fn escaped_unit(bytes: &[u8]) -> Option<u16> {
    let digits = bytes.strip_prefix(b"\\u")?.get(..4)?;
    u16::from_str_radix(std::str::from_utf8(digits).ok()?, 16).ok()
}
/// Adapt only unpaired UTF-16 escapes, leaving syntax checking to the JSON library.
pub(super) fn string_units(text: &str) -> String {
    let mut remaining = text.as_bytes();
    let mut output = Vec::with_capacity(remaining.len());
    while let Some((&byte, tail)) = remaining.split_first() {
        if byte != b'\\' {
            output.push(byte);
            remaining = tail;
            continue;
        }
        let Some(unit) = escaped_unit(remaining) else {
            let length = remaining.len().min(2);
            output.extend_from_slice(&remaining[..length]);
            remaining = &remaining[length..];
            continue;
        };
        let paired = (0xd800..=0xdbff).contains(&unit)
            && escaped_unit(&remaining[6..]).is_some_and(|next| (0xdc00..=0xdfff).contains(&next));
        let length = if paired { 12 } else { 6 };
        if !paired && (0xd800..=0xdfff).contains(&unit) {
            output.extend_from_slice(b"\\ufffd");
        } else {
            output.extend_from_slice(&remaining[..length]);
        }
        remaining = &remaining[length..];
    }
    // The input is UTF-8; substitutions and escape copies retain its scalar boundaries.
    String::from_utf8_lossy(&output).into_owned()
}

/// Read accepted object/array collections, with canonical integer keys before other keys.
pub(super) fn entries(raw: Option<&RawValue>) -> Vec<(EntryKey, &RawValue)> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    let mut entries: Vec<_> = if raw.get().starts_with('[') {
        serde_json::from_str::<Vec<&RawValue>>(raw.get())
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .map(|(index, value)| (EntryKey(index.to_string().into_bytes()), value))
            .collect()
    } else {
        serde_json::from_str::<indexmap::IndexMap<EntryKey, &RawValue>>(raw.get())
            .unwrap_or_default()
            .into_iter()
            .collect()
    };
    entries.sort_by_key(|(key, _)| {
        std::str::from_utf8(&key.0)
            .ok()
            .and_then(crate::providers::json_text::array_index)
            .unwrap_or(u32::MAX)
    });
    entries
}
