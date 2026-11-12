//! Table boundaries and balanced inline delimiters.
use maestro_text::is_whitespace_scalar;
use std::ops::Range;

/// Split only unescaped pipes, retaining authored cell boundaries.
pub(crate) fn cell_ranges(row: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    let mut slashes = 0;
    for (offset, byte) in row.bytes().enumerate() {
        if byte == b'|' && slashes % 2 == 0 {
            ranges.push(start..offset);
            start = offset + 1;
        }
        slashes = if byte == b'\\' { slashes + 1 } else { 0 };
    }
    ranges.push(start..row.len());
    if ranges.first().is_some_and(|range| {
        row[range.clone()]
            .trim_matches(is_whitespace_scalar)
            .is_empty()
    }) {
        ranges.remove(0);
    }
    if ranges.last().is_some_and(|range| {
        row[range.clone()]
            .trim_matches(is_whitespace_scalar)
            .is_empty()
    }) {
        ranges.pop();
    }
    ranges
}

/// Trim cells, unescape their pipes and apply the requested column count.
pub(crate) fn split_cells(row: &str, count: Option<usize>) -> Vec<String> {
    let mut cells: Vec<_> = cell_ranges(row)
        .into_iter()
        .map(|range| {
            row[range]
                .trim_matches(is_whitespace_scalar)
                .replace("\\|", "|")
        })
        .collect();
    if let Some(count) = count.filter(|count| *count != 0) {
        cells.resize(count, String::new());
    }
    cells
}

/// Remove a suffix containing only the selected scalar.
pub(crate) fn rtrim(source: &str, scalar: char) -> &str {
    source.trim_end_matches(scalar)
}

/// Result of walking escaped, balanced delimiter pairs.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ClosingBracket {
    /// First closer without a matching opener, in UTF-8 bytes.
    Found(usize),
    /// No unmatched closer.
    Missing,
    /// An opener remains after the walk.
    Unclosed,
}

/// Locate an unmatched closer without interpreting escaped delimiters.
pub(crate) fn find_closing_bracket(
    source: &str,
    pair: (char, char),
    options: crate::MarkedOptions,
) -> ClosingBracket {
    if !source.contains(pair.1) {
        return ClosingBracket::Missing;
    }
    let mut depth = 0_usize;
    let mut scalars = source.char_indices();
    while let Some((offset, scalar)) = scalars.next() {
        if scalar == '\\' {
            scalars.next();
        } else if scalar == pair.0 {
            depth += 1;
        } else if scalar == pair.1 {
            let Some(remaining) = depth.checked_sub(1) else {
                return ClosingBracket::Found(offset);
            };
            depth = remaining;
        }
    }
    if depth > 0 && options != crate::MarkedOptions::Export {
        ClosingBracket::Unclosed
    } else {
        ClosingBracket::Missing
    }
}
