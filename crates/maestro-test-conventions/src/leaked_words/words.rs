//! Splitting text into words and matching them against the denied lists.

use std::ops::Range;

/// A denied word found in a text.
#[derive(Clone)]
pub(super) struct Found {
    /// What an exemption row names: the `DENIED` word, or the hash in sixteen hex digits.
    pub(super) key: String,
    /// The `DENIED` word, or a hashed word as the text spells it.
    pub(super) word: String,
    /// Byte offset in the text where the word starts.
    pub(super) offset: usize,
}

/// The 64-bit FNV-1a hash of a word's UTF-8 bytes.
pub(super) fn fnv1a(word: &str) -> u64 {
    word.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Every denied word in a text, in text order.
///
/// A readable word matches when its `_`-joined segments appear next to each other, joined
/// by `_`, `-` or a case change alone; a hashed word matches a single segment whose lowercase form
/// hashes to it.
pub(super) fn find(text: &str, denied: &[&str], hashed: &[u64]) -> Vec<Found> {
    let spans = spans(text);
    let parts: Vec<String> = spans
        .iter()
        .map(|span| text[span.clone()].to_lowercase())
        .collect();
    let mut found = Vec::new();
    for (span, part) in spans.iter().zip(&parts) {
        let hash = fnv1a(part);
        if hashed.contains(&hash) {
            found.push(Found {
                key: format!("{hash:#018x}"),
                word: text[span.clone()].to_owned(),
                offset: span.start,
            });
        }
    }
    for word in denied {
        let wanted: Vec<&str> = word.split('_').collect();
        for (start, window) in parts.windows(wanted.len()).enumerate() {
            let run = &spans[start..start + wanted.len()];
            if window.iter().zip(&wanted).all(|(part, want)| part == want) && joined(text, run) {
                found.push(Found {
                    key: (*word).to_owned(),
                    word: (*word).to_owned(),
                    offset: run[0].start,
                });
            }
        }
    }
    found.sort_by_key(|found| found.offset);
    found
}

/// Whether only `_` or `-` (or a case change) joins consecutive spans.
fn joined(text: &str, run: &[Range<usize>]) -> bool {
    run.windows(2).all(|pair| {
        text[pair[0].end..pair[1].start]
            .chars()
            .all(|c| c == '_' || c == '-')
    })
}

/// The byte ranges of a text's segments.
pub(super) fn spans(text: &str) -> Vec<Range<usize>> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut spans = Vec::new();
    let mut start = None;
    for (index, &(offset, c)) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            if let Some(from) = start.take() {
                spans.push(from..offset);
            }
        } else if let Some(from) = start {
            if begins_segment(&chars, index) {
                spans.push(from..offset);
                start = Some(offset);
            }
        } else {
            start = Some(offset);
        }
    }
    if let Some(from) = start {
        spans.push(from..text.len());
    }
    spans
}

/// Whether the character at `index` starts a new segment inside a run of alphanumerics.
///
/// An uppercase letter starts one after a lowercase letter, and when it ends an acronym
/// before a lowercase letter.
fn begins_segment(chars: &[(usize, char)], index: usize) -> bool {
    let c = chars[index].1;
    let previous = chars[index - 1].1;
    let after_lower = previous.is_lowercase() && c.is_uppercase();
    let ends_acronym = c.is_uppercase()
        && previous.is_uppercase()
        && chars
            .get(index + 1)
            .is_some_and(|next| next.1.is_lowercase());
    after_lower || ends_acronym
}
