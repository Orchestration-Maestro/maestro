//! Header values at the HTTP seam: request text becomes bytes, response bytes become text.

use std::collections::BTreeMap;

use indexmap::IndexMap;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

/// Whitespace the HTTP field syntax removes around a value: tab, line feed, carriage return
/// and space; no other character is whitespace here.
fn edge_whitespace(character: char) -> bool {
    matches!(character, '\t' | '\n' | '\r' | ' ')
}

/// The bytes a value names: one byte per character, which must not exceed U+00FF.
fn octets(name: &str, value: &str) -> Result<Vec<u8>, String> {
    value
        .chars()
        .map(|character| {
            u8::try_from(character).map_err(|_| {
                let code = u32::from(character);
                format!("Header {name} holds U+{code:04X}, which is not a single byte.")
            })
        })
        .collect()
}

/// Trim every value at its HTTP edges, in place, and convert the headers for sending.
///
/// Values are text whose characters each name one byte. Empty values are kept.
///
/// # Errors
/// Fails with the text of the first header that has a character above U+00FF, an invalid
/// name, or a value that cannot be sent.
pub(super) fn request_pairs(
    headers: &mut IndexMap<String, String>,
) -> Result<Vec<(HeaderName, HeaderValue)>, String> {
    headers
        .iter_mut()
        .map(|(name, value)| {
            *value = value.trim_matches(edge_whitespace).to_owned();
            let header = HeaderName::from_bytes(name.as_bytes())
                .map_err(|error| format!("Header {name}: {error}"))?;
            let content = HeaderValue::from_bytes(&octets(name, value)?)
                .map_err(|error| format!("Header {name}: {error}"))?;
            Ok((header, content))
        })
        .collect()
}

/// A response value as text: each native byte is the character of that number.
#[cfg(not(target_arch = "wasm32"))]
fn text(value: &HeaderValue) -> String {
    value.as_bytes().iter().copied().map(char::from).collect()
}

/// A response value as text: the browser has decoded it already and carries it as UTF-8.
#[cfg(target_arch = "wasm32")]
fn text(value: &HeaderValue) -> String {
    String::from_utf8_lossy(value.as_bytes()).into_owned()
}

/// Response headers as one text per name. Repeated values keep their order and are joined
/// with `, `; `cookie` values are joined with `; ` and only the last `set-cookie` is kept.
pub(super) fn response_record(headers: &HeaderMap) -> BTreeMap<String, String> {
    headers
        .keys()
        .map(|name| {
            let mut values = headers.get_all(name).iter().map(text);
            let value = match name.as_str() {
                "set-cookie" => values.next_back().unwrap_or_default(),
                "cookie" => values.collect::<Vec<_>>().join("; "),
                _ => values.collect::<Vec<_>>().join(", "),
            };
            (name.as_str().to_owned(), value)
        })
        .collect()
}
