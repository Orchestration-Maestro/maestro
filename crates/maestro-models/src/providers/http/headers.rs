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

/// The header name `name`, which must be a valid token.
fn header_name(name: &str) -> Result<HeaderName, String> {
    HeaderName::from_bytes(name.as_bytes()).map_err(|error| format!("Header {name}: {error}"))
}

/// Trim every value at its HTTP edges, in place, and check the headers against the Fetch
/// Standard's header rules.
///
/// Values are text whose characters each name one byte. Empty values are kept. Whether a
/// client can carry a value is that client's rule; see [`client_pairs`].
///
/// # Errors
/// Fails with the text of the first header that has an invalid name, a character above
/// U+00FF, or a value holding NUL, a carriage return or a line feed.
pub(super) fn normalize_request(headers: &mut IndexMap<String, String>) -> Result<(), String> {
    for (name, value) in headers {
        *value = value.trim_matches(edge_whitespace).to_owned();
        header_name(name)?;
        octets(name, value)?;
        if value.contains(['\0', '\r', '\n']) {
            return Err(format!("Header {name} holds NUL or a line break."));
        }
    }
    Ok(())
}

/// Normalize the headers and convert them for the default client, whose header type accepts a
/// value only if every byte is a tab, a printable character or at least 0x80: any other control
/// character, and DEL, is not carried.
///
/// # Errors
/// Fails with the text of the first header that [`normalize_request`] rejects or whose value
/// the default client cannot carry.
pub(super) fn client_pairs(
    headers: &mut IndexMap<String, String>,
) -> Result<Vec<(HeaderName, HeaderValue)>, String> {
    normalize_request(headers)?;
    headers
        .iter()
        .map(|(name, value)| {
            let content = HeaderValue::from_bytes(&octets(name, value)?)
                .map_err(|error| format!("Header {name}: {error}"))?;
            Ok((header_name(name)?, content))
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
