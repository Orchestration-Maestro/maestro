//! Data URL bytes with exact source identity.

use base64::Engine as _;

use super::{Cache, Failure, Source, notmodified::NotModified};
use crate::providers::base64_text::FORGIVING;

/// Decode the selected data URL after comparing its exact identity.
pub(super) fn load(uri: &url::Url, cache: Option<&Cache>) -> Result<Source, Failure> {
    if matches!(cache, Some(Cache::Data(previous)) if previous == uri.as_str()) {
        return Err(NotModified.into());
    }
    let (metadata, payload) = uri
        .as_str()
        .split_once(',')
        .ok_or_else(|| std::io::Error::other("malformed data: URI"))?;
    let bytes = unescape(payload);
    let bytes = if metadata.split(';').skip(1).any(|part| part == "base64") {
        let mut text: Vec<u8> = bytes
            .into_iter()
            .take_while(|byte| *byte != b'=')
            .filter_map(|byte| match byte {
                b'-' => Some(b'+'),
                b'_' => Some(b'/'),
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/' => Some(byte),
                _ => None,
            })
            .collect();
        if text.len() % 4 == 1 {
            text.pop();
        }
        FORGIVING.decode(text)?
    } else {
        bytes
    };
    Ok(Source {
        bytes,
        cache: Cache::Data(uri.to_string()),
    })
}

/// Decode legacy byte escapes without rejecting incomplete percent sequences.
fn unescape(text: &str) -> Vec<u8> {
    let mut bytes = text.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    while let Some((&first, rest)) = bytes.split_first() {
        let (start, width) = if bytes.starts_with(b"%u") {
            (2, 6)
        } else {
            (1, 3)
        };
        let decoded = (first == b'%')
            .then(|| bytes.get(..width))
            .flatten()
            .and_then(|escape| std::str::from_utf8(&escape[start..]).ok())
            .and_then(|digits| u16::from_str_radix(digits, 16).ok());
        if let Some(value) = decoded {
            output.push(value.to_le_bytes()[0]);
            bytes = &bytes[width..];
        } else {
            output.push(first);
            bytes = rest;
        }
    }
    output
}
