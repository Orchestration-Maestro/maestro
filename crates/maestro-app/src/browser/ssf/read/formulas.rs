//! Sheet-local formula metadata with lexical coordinate displacement.
use super::super::{CellAddress, Range, WorkbookError, decode_cell, decode_range, encode_col};
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
    let text = super::super::unescapexml::unescapexml(&node.text_content())
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

/// Advance complete coordinate runs outside opaque spans.
fn expand(text: &str, start: CellAddress, end: CellAddress) -> Result<String, WorkbookError> {
    let mut result = String::with_capacity(text.len());
    let mut cursor = 0;
    while cursor < text.len() {
        let rest = &text[cursor..];
        let length = match rest.as_bytes()[0] {
            b'\'' | b'"' => quoted_length(rest, rest.as_bytes()[0]),
            b'[' => bracket_length(rest),
            _ => 0,
        };
        if length > 0 {
            result.push_str(&rest[..length]);
            cursor += length;
            continue;
        }
        let length = run_length(rest);
        if length == 0 {
            if let Some(c) = rest.chars().next() {
                result.push(c);
                cursor += c.len_utf8();
            }
            continue;
        }
        let run = &rest[..length];
        let following = &rest[length..];
        let qualifier = if run.starts_with('#') {
            0
        } else {
            qualifier_length(rest, length, text[..cursor].ends_with('!'))
        };
        if run.starts_with('#') || qualifier > 0 || following.starts_with('(') {
            let length = length.max(qualifier);
            result.push_str(&rest[..length]);
            cursor += length;
            continue;
        }
        let coordinate = run.strip_suffix('#').unwrap_or(run);
        if coordinate_length(coordinate) == Some(coordinate.len()) {
            result.push_str(&displace(coordinate, start, end)?);
            result.push_str(&run[coordinate.len()..]);
        } else {
            result.push_str(run);
        }
        cursor += length;
    }
    Ok(result)
}

/// Maximal operand runs end at operators, whitespace or operand punctuation.
fn run_length(text: &str) -> usize {
    text.find(|c: char| c.is_whitespace() || ":,^*/+-&=<>%(){}[]!\"';@".contains(c))
        .unwrap_or(text.len())
}

/// A colon joins unquoted sheet qualifiers unless the first run follows a bang.
fn qualifier_length(text: &str, length: usize, after_bang: bool) -> usize {
    let rest = &text[length..];
    if rest.starts_with('!') {
        return length + 1;
    }
    if !after_bang && let Some(right) = rest.strip_prefix(':') {
        let second = run_length(right);
        if second > 0 && right[second..].starts_with('!') {
            return length + 1 + second + 1;
        }
    }
    0
}

/// Match the bounded coordinate core without consuming either neighbour.
fn coordinate_length(text: &str) -> Option<usize> {
    let column_start = usize::from(text.starts_with('$'));
    let column_length = text[column_start..]
        .bytes()
        .take(4)
        .take_while(u8::is_ascii_alphabetic)
        .count();
    let column_end = column_start + column_length;
    let column = text[column_start..column_end].to_ascii_uppercase();
    if column_length == 0 || column_length > 3 || (column_length == 3 && column.as_str() > "XFD") {
        return None;
    }
    let row_start = column_end + usize::from(text[column_end..].starts_with('$'));
    let row_length = text[row_start..]
        .bytes()
        .take(8)
        .take_while(u8::is_ascii_digit)
        .count();
    let row = &text[row_start..row_start + row_length];
    if row.starts_with('0') || !(1..=1_048_576).contains(&row.parse::<u32>().ok()?) {
        return None;
    }
    Some(row_start + row_length)
}

/// Advance coordinates through the shared address owner, preserving inactive axes and case.
fn displace(text: &str, start: CellAddress, end: CellAddress) -> Result<String, WorkbookError> {
    let row = text
        .trim_start_matches('$')
        .trim_start_matches(|c: char| c.is_ascii_alphabetic());
    if (text.starts_with('$') || start.c == end.c) && (row.starts_with('$') || start.r == end.r) {
        return Ok(text.to_owned());
    }
    let upper = text.to_ascii_uppercase();
    let address = decode_cell(&upper)?;
    let c = i64::from(address.c)
        + if text.starts_with('$') {
            0
        } else {
            i64::from(end.c) - i64::from(start.c)
        };
    let r = i64::from(address.r)
        + if row.starts_with('$') {
            0
        } else {
            i64::from(end.r) - i64::from(start.r)
        };
    let column =
        u32::try_from(c).map_err(|_| WorkbookError::Reader(format!("invalid column {c}")))?;
    let shifted = format!(
        "{}{}{}{}",
        if text.starts_with('$') { "$" } else { "" },
        encode_col(column),
        if row.starts_with('$') { "$" } else { "" },
        r + 1
    );
    Ok(retain_column_case(text, &shifted))
}

/// Apply authored column letter casing, extending the last case at a width change.
fn retain_column_case(text: &str, shifted: &str) -> String {
    let mut casing = text.bytes().filter(u8::is_ascii_alphabetic);
    let mut last = b'A';
    shifted
        .chars()
        .map(|c| {
            if c.is_ascii_alphabetic() {
                last = casing.next().unwrap_or(last);
                if last.is_ascii_lowercase() {
                    return c.to_ascii_lowercase();
                }
            }
            c
        })
        .collect()
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
