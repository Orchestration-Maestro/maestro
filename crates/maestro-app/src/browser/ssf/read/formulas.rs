//! Sheet-local formula metadata with operand-aware native displacement.
use super::super::{CellAddress, Range, WorkbookError, decode_range};
use crate::browser::docx_preview::{XmlElement, XmlParser};
use std::collections::BTreeMap;

/// Shared templates retain their original coordinates until the sheet is complete.
pub(super) type Shared = BTreeMap<String, (CellAddress, String)>;

/// Decode authored formula text once and expand only reference operands.
pub(super) fn formula(
    xml: &XmlParser<'_>,
    node: XmlElement<'_, '_>,
    position: CellAddress,
    shared: &mut Shared,
) -> Result<Option<String>, WorkbookError> {
    let Some(node) = xml.element(node, "f") else {
        return Ok(None);
    };
    let text = super::string_cell(&node.text_content())
        .replace("\r\n", "\n")
        .replace("_xlfn.", "");
    if xml.attr(node, "t") == Some("shared") {
        let id = xml.attr(node, "si").unwrap_or("");
        if text.is_empty() {
            return shared
                .get(id)
                .map(|(start, text)| expand(text, *start, position))
                .transpose();
        }
        shared.insert(id.to_owned(), (position, text.clone()));
    }
    Ok((!text.is_empty()).then_some(text))
}

/// Classify operands before delegating coordinate arithmetic to the native library.
fn expand(text: &str, start: CellAddress, end: CellAddress) -> Result<String, WorkbookError> {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        let first = rest.as_bytes()[0];
        let length = if first == b'\'' || first == b'"' {
            quoted_length(rest, first)
        } else if first == b'[' {
            bracket_length(rest)
        } else {
            rest.bytes()
                .take_while(|c| c.is_ascii_alphanumeric() || b"\\._$:".contains(c))
                .count()
        };
        if length == 0 {
            let length = rest.chars().next().map_or(0, char::len_utf8);
            result.push_str(&rest[..length]);
            rest = &rest[length..];
            continue;
        }
        let (token, remaining) = rest.split_at(length);
        if matches!(first, b'\'' | b'"' | b'[') || remaining.starts_with(['!', '(', '[']) {
            result.push_str(token);
        } else {
            result.push_str(
                &calamine::expand_shared_formula(token, (start.r, start.c), (end.r, end.c))
                    .map_err(|e| WorkbookError::Reader(e.to_string()))?,
            );
        }
        rest = remaining;
    }
    Ok(result)
}

/// Keep quoted string and sheet-name contents opaque, including doubled delimiters.
fn quoted_length(text: &str, quote: u8) -> usize {
    let mut bytes = text.bytes().enumerate().skip(1).peekable();
    while let Some((index, byte)) = bytes.next() {
        if byte == quote {
            if bytes.peek().is_some_and(|(_, next)| *next == quote) {
                bytes.next();
            } else {
                return index + 1;
            }
        }
    }
    text.len()
}

/// Structured references nest brackets and escape special column-name characters.
fn bracket_length(text: &str) -> usize {
    let mut depth = 0;
    let mut bytes = text.bytes().enumerate();
    while let Some((index, byte)) = bytes.next() {
        match byte {
            b'\'' => {
                bytes.next();
            }
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return index + 1;
                }
            }
            _ => {}
        }
    }
    text.len()
}

/// Retain encountered array regions and select the last covering range.
pub(super) fn array_range(
    xml: &XmlParser<'_>,
    node: XmlElement<'_, '_>,
    position: CellAddress,
    arrays: &mut Vec<(Range, String)>,
) -> Result<Option<String>, WorkbookError> {
    let own = xml
        .element(node, "f")
        .filter(|f| xml.attr(*f, "t") == Some("array") && !f.text_content().is_empty())
        .and_then(|f| xml.attr(f, "ref"));
    if let Some(range) = own.filter(|r| r.contains(':')) {
        arrays.push((decode_range(range)?, range.to_owned()));
    }
    Ok(arrays
        .iter()
        .rev()
        .find(|(range, _)| {
            (range.s.r..=range.e.r).contains(&position.r)
                && (range.s.c..=range.e.c).contains(&position.c)
        })
        .map(|(_, text)| text.clone())
        .or_else(|| own.map(str::to_owned)))
}
