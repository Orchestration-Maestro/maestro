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

/// Traverse borrowed operand spans, copying all non-coordinate source bytes.
/// Calls/groups need no retained stack because enclosing groups do not affect
/// classification; only opaque arrays retain delimiter context.
fn expand(text: &str, start: CellAddress, end: CellAddress) -> Result<String, WorkbookError> {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        let (length, kind) = operand(rest);
        let (token, remaining) = rest.split_at(length);
        if let Some(kind) = kind {
            result.push_str(&displace(token, kind, start, end)?);
        } else {
            result.push_str(token);
        }
        rest = remaining;
    }
    Ok(result)
}

/// Coordinate productions distinguished from names and scalar numbers.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ReferenceKind {
    /// A column and a row.
    Cell,
    /// A whole column endpoint, admitted only in a homogeneous pair.
    Column,
    /// A whole row endpoint, admitted only in a homogeneous pair.
    Row,
}

/// Select complete qualifiers and opaque productions before coordinate operands.
fn operand(text: &str) -> (usize, Option<ReferenceKind>) {
    if let Some(length) = qualifier_length(text) {
        return (length, None);
    }
    match text.as_bytes()[0] {
        b'\'' | b'"' => return (quoted_length(text, text.as_bytes()[0]), None),
        b'[' => return (bracket_length(text), None),
        b'{' => return (array_length(text), None),
        b'#' => return (error_length(text), None),
        _ => {}
    }
    let length = name_length(text);
    let token = &text[..length];
    let suffix = &text[length..];
    if suffix.starts_with('[') && !token.is_empty() {
        return (length + bracket_length(suffix), None);
    }
    if let Some(kind) = reference_kind(token) {
        if let Some(pair) = axis_pair(text, length, kind) {
            return (pair, Some(kind));
        }
        if kind == ReferenceKind::Cell
            && terminates(suffix)
            && !suffix.trim_start().starts_with('(')
        {
            return (length, Some(kind));
        }
    }
    if let Some(length) = number_length(text).filter(|n| terminates(&text[*n..])) {
        return (length, None);
    }
    if token.chars().next().is_some_and(name_start) && terminates(suffix) {
        return (length, None);
    }
    (recovery_length(text), None)
}

/// A colon belongs to an axis operand only with two complete same-axis endpoints.
fn axis_pair(text: &str, length: usize, kind: ReferenceKind) -> Option<usize> {
    if kind == ReferenceKind::Cell {
        return None;
    }
    let right = text[length..].strip_prefix(':')?;
    let length_right = name_length(right);
    (reference_kind(&right[..length_right]) == Some(kind)
        && terminates(&right[length_right..])
        && !right[length_right..].trim_start().starts_with('('))
    .then_some(length + 1 + length_right)
}

/// Recognize A1 syntax; the native library owns grid-bound validation.
fn reference_kind(text: &str) -> Option<ReferenceKind> {
    let text = text.strip_prefix('$').unwrap_or(text);
    let boundary = text.bytes().take_while(u8::is_ascii_alphabetic).count();
    let (column, row) = text.split_at(boundary);
    let row = row.strip_prefix('$').unwrap_or(row);
    if column.len() > 3
        || row.len() > 7
        || row.starts_with('0')
        || !row.bytes().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    match (column.is_empty(), row.is_empty()) {
        (false, false) => Some(ReferenceKind::Cell),
        (false, true) if !text.ends_with('$') => Some(ReferenceKind::Column),
        (true, false) => Some(ReferenceKind::Row),
        _ => None,
    }
}

/// Preserve inactive axes and delegate only isolated coordinate operations.
fn displace(
    text: &str,
    kind: ReferenceKind,
    start: CellAddress,
    end: CellAddress,
) -> Result<String, WorkbookError> {
    let active = |part| applicable(part, kind, start, end);
    if !text.split(':').any(active)
        || calamine::expand_shared_formula(text, (0, 0), (0, 0)).is_err()
    {
        return Ok(text.to_owned());
    }
    let shifted = calamine::expand_shared_formula(text, (start.r, start.c), (end.r, end.c))
        .map_err(|e| WorkbookError::Reader(e.to_string()))?;
    Ok(text
        .split(':')
        .zip(shifted.split(':'))
        .map(|(raw, moved)| if active(raw) { moved } else { raw })
        .collect::<Vec<_>>()
        .join(":"))
}

/// An operand changes only when at least one relative axis has a nonzero delta.
fn applicable(text: &str, kind: ReferenceKind, start: CellAddress, end: CellAddress) -> bool {
    match kind {
        ReferenceKind::Column => !text.starts_with('$') && start.c != end.c,
        ReferenceKind::Row => !text.starts_with('$') && start.r != end.r,
        ReferenceKind::Cell => {
            let row = text
                .trim_start_matches('$')
                .trim_start_matches(|c: char| c.is_ascii_alphabetic());
            (!text.starts_with('$') && start.c != end.c)
                || (!row.starts_with('$') && start.r != end.r)
        }
    }
}

/// Non-ASCII format characters belong to names without Unicode normalization.
fn name_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c >= '\u{80}' || matches!(c, '_' | '\\')
}

/// Complete defined-name continuation, including coordinate-shaped suffixes.
fn name_character(c: char) -> bool {
    name_start(c) || c.is_ascii_digit() || matches!(c, '.' | '?')
}

/// End of a name or coordinate spelling, before its production's successor.
fn name_length(text: &str) -> usize {
    text.find(|c| !name_character(c) && c != '$')
        .unwrap_or(text.len())
}

/// Unquoted sheet text permits coordinate spellings without classifying them.
fn sheet_length(text: &str) -> usize {
    text.find(|c: char| c.is_ascii_whitespace() || ":,+-*/^&=<>%;(){}!\"'[]\\?".contains(c))
        .unwrap_or(text.len())
}

/// A sheet or external-book qualifier is opaque only when complete through `!`.
fn qualifier_length(text: &str) -> Option<usize> {
    if text.starts_with('\'') {
        let length = quoted_length(text, b'\'');
        return text[length..].starts_with('!').then_some(length + 1);
    }
    let book = if text.starts_with('[') {
        text.find(']')? + 1
    } else {
        0
    };
    let first = sheet_length(&text[book..]);
    let mut length = book + first;
    if text[length..].starts_with(':') {
        let second = sheet_length(&text[length + 1..]);
        if first == 0 || second == 0 {
            return None;
        }
        length += 1 + second;
    }
    text[length..].starts_with('!').then_some(length + 1)
}

/// Operators, separators and group delimiters terminate independent operands.
fn terminates(text: &str) -> bool {
    text.is_empty()
        || text.starts_with(|c: char| c.is_ascii_whitespace() || ":,+-*/^&=<>%;(){}@#".contains(c))
}

/// Consume a complete decimal/exponent literal, including an exponent sign.
fn number_length(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut cursor = bytes.iter().take_while(|c| c.is_ascii_digit()).count();
    let mut digits = cursor;
    if bytes.get(cursor) == Some(&b'.') {
        cursor += 1;
        let fraction = bytes[cursor..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count();
        cursor += fraction;
        digits += fraction;
    }
    if digits == 0 {
        return None;
    }
    if matches!(bytes.get(cursor), Some(b'e' | b'E')) {
        let mut exponent = cursor + 1;
        if matches!(bytes.get(exponent), Some(b'+' | b'-')) {
            exponent += 1;
        }
        let count = bytes[exponent..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count();
        if count != 0 {
            cursor = exponent + count;
        }
    }
    Some(cursor)
}

/// Error literals retain their slash, question mark and exclamation mark.
fn error_length(text: &str) -> usize {
    text[1..]
        .find(|c: char| c.is_ascii_whitespace() || ":,+-*^&=<>%;(){}@#".contains(c))
        .map_or(text.len(), |n| n + 1)
}

/// Unknown units are never rescanned internally; colon also ends recovery.
fn recovery_length(text: &str) -> usize {
    let mut cursor = 0;
    while cursor < text.len() {
        let remaining = &text[cursor..];
        if terminates(remaining) {
            return if cursor == 0 {
                remaining.chars().next().map_or(0, char::len_utf8)
            } else {
                cursor
            };
        }
        cursor += match remaining.as_bytes()[0] {
            b'\'' | b'"' => quoted_length(remaining, remaining.as_bytes()[0]),
            b'[' => bracket_length(remaining),
            _ => remaining.chars().next().map_or(0, char::len_utf8),
        };
    }
    cursor
}

/// Arrays contain constants, retained with iterative delimiter context.
fn array_length(text: &str) -> usize {
    let mut delimiters = Vec::new();
    let mut cursor = 0;
    while cursor < text.len() {
        let remaining = &text[cursor..];
        match remaining.as_bytes()[0] {
            b'"' => {
                cursor += quoted_length(remaining, b'"');
                continue;
            }
            b'{' => delimiters.push(b'}'),
            b'}' => {
                delimiters.pop();
                if delimiters.is_empty() {
                    return cursor + 1;
                }
            }
            _ => {}
        }
        cursor += remaining.chars().next().map_or(0, char::len_utf8);
    }
    text.len()
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
