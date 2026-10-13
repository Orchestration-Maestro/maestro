//! Original-snapshot edit matching: line endings, BOM, fuzzy normalization, unique
//! matching, application of disjoint edits, numbered diffs and read-only previews.
//!
//! Matching positions and lengths are UTF-8 byte offsets into the text they describe.

mod numbered;
mod preview;
mod text_diff;

pub use numbered::{EditDiffResult, generate_diff_string};
pub use preview::{EditDiffError, compute_edit_diff, compute_edits_diff};

use crate::utils::diagnostics::failure;
use icu_normalizer::ComposingNormalizer;
use maestro_models::DiagnosticErrorInfo;
use maestro_text::is_whitespace_scalar;
pub use maestro_text::normalize_to_lf;
use std::{borrow::Cow, cell::OnceCell};

/// The line-ending convention of a text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEnding {
    /// Lines end with LF.
    Lf,
    /// Lines end with CR LF.
    CrLf,
}

/// The convention indicated by the first LF; a text without LF uses LF.
#[must_use]
pub fn detect_line_ending(content: &str) -> LineEnding {
    match content.find('\n') {
        Some(lf) if content[..lf].ends_with('\r') => LineEnding::CrLf,
        _ => LineEnding::Lf,
    }
}

/// In CRLF mode replace each LF with CR LF, leaving existing CR untouched; LF mode keeps `text`.
#[must_use]
pub fn restore_line_endings(text: &str, ending: LineEnding) -> String {
    match ending {
        LineEnding::Lf => text.to_owned(),
        LineEnding::CrLf => text.replace('\n', "\r\n"),
    }
}

/// Fold one scalar of already compatibility-normalized text.
fn fold_scalar(scalar: char) -> char {
    match scalar {
        '\u{2018}'..='\u{201b}' => '\'',
        '\u{201c}'..='\u{201f}' => '"',
        '\u{2010}'..='\u{2015}' | '\u{2212}' => '-',
        '\u{a0}' | '\u{2002}'..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}' => ' ',
        other => other,
    }
}

/// NFKC, then per-LF-line trailing whitespace removal, then quote, dash and space folding.
///
/// Trailing whitespace follows [`maestro_text::is_whitespace_scalar`].
#[must_use]
pub fn normalize_for_fuzzy_match(text: &str) -> String {
    let composed = ComposingNormalizer::new_nfkc().normalize(text);
    let mut folded = String::with_capacity(composed.len());
    for (index, line) in composed.split('\n').enumerate() {
        if index > 0 {
            folded.push('\n');
        }
        let kept = line.trim_end_matches(is_whitespace_scalar);
        folded.extend(kept.chars().map(fold_scalar));
    }
    folded
}

/// Split one leading BOM from `content`, returning `(bom, text)`.
#[must_use]
pub fn strip_bom(content: &str) -> (&str, &str) {
    match content.strip_prefix('\u{feff}') {
        Some(text) => ("\u{feff}", text),
        None => ("", content),
    }
}

/// One authored replacement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    /// Text to find.
    pub old_text: String,
    /// Text to put in its place.
    pub new_text: String,
}

/// The comparison base and the completed replacement text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppliedEditsResult {
    /// The text every edit was matched against.
    pub base_content: String,
    /// The text after all replacements.
    pub new_content: String,
}

/// The outcome of looking for one needle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FuzzyMatchResult<'a> {
    /// The needle occurs in `content_for_replacement`, which is the original content for
    /// an exact match and its fuzzy normalization otherwise.
    Found {
        /// Byte offset of the first match.
        index: usize,
        /// Byte length of the match.
        match_length: usize,
        /// Whether the match needed fuzzy normalization.
        used_fuzzy_match: bool,
        /// The text `index` and `match_length` describe.
        content_for_replacement: Cow<'a, str>,
    },
    /// No match; the original content is retained.
    NotFound {
        /// The content that was searched.
        content_for_replacement: &'a str,
    },
}

/// A text and its lazily computed fuzzy normalization.
struct Space<'a> {
    /// The text itself.
    text: &'a str,
    /// Its fuzzy normalization, computed on first use.
    folded: OnceCell<String>,
}

impl<'a> Space<'a> {
    /// A space over `text`.
    fn new(text: &'a str) -> Self {
        Self {
            text,
            folded: OnceCell::new(),
        }
    }

    /// The fuzzy normalization of the text.
    fn folded(&self) -> &str {
        self.folded
            .get_or_init(|| normalize_for_fuzzy_match(self.text))
    }

    /// First match: exact first, then in normalized space. A nonempty needle that
    /// normalization erases matches only exactly.
    fn find(&self, old: &str, folded_old: &str) -> Option<Hit> {
        if let Some(index) = self.text.find(old) {
            return Some(Hit {
                index,
                length: old.len(),
                fuzzy: false,
            });
        }
        let index = self
            .folded()
            .find(folded_old)
            .filter(|_| !folded_old.is_empty())?;
        Some(Hit {
            index,
            length: folded_old.len(),
            fuzzy: true,
        })
    }

    /// Occurrence count including overlaps, in normalized space unless the needle vanishes.
    fn occurrences(&self, old: &str, folded_old: &str) -> usize {
        if folded_old.is_empty() {
            count_overlapping(self.text, old)
        } else {
            count_overlapping(self.folded(), folded_old)
        }
    }
}

/// A located needle.
struct Hit {
    /// Byte offset.
    index: usize,
    /// Byte length.
    length: usize,
    /// Whether it lies in normalized space.
    fuzzy: bool,
}

/// Count occurrences of the nonempty `needle`, overlapping ones included, in linear time.
fn count_overlapping(haystack: &str, needle: &str) -> usize {
    let needle = needle.as_bytes();
    let mut border = vec![0usize; needle.len()];
    let mut matched = 0;
    for at in 1..needle.len() {
        while matched > 0 && needle[at] != needle[matched] {
            matched = border[matched - 1];
        }
        if needle[at] == needle[matched] {
            matched += 1;
        }
        border[at] = matched;
    }
    let mut count = 0;
    matched = 0;
    for &byte in haystack.as_bytes() {
        while matched > 0 && (matched == needle.len() || byte != needle[matched]) {
            matched = border[matched - 1];
        }
        if byte == needle[matched] {
            matched += 1;
        }
        if matched == needle.len() {
            count += 1;
        }
    }
    count
}

/// Find `old_text` in `content`, exact first, then in fuzzy-normalized space.
///
/// An empty needle matches exactly at offset 0. A nonempty needle that normalization
/// erases is only matched literally, never as an empty insertion point.
#[must_use]
pub fn fuzzy_find_text<'a>(content: &'a str, old_text: &str) -> FuzzyMatchResult<'a> {
    let space = Space::new(content);
    let folded_old = normalize_for_fuzzy_match(old_text);
    match space.find(old_text, &folded_old) {
        None => FuzzyMatchResult::NotFound {
            content_for_replacement: content,
        },
        Some(Hit {
            index,
            length,
            fuzzy,
        }) => FuzzyMatchResult::Found {
            index,
            match_length: length,
            used_fuzzy_match: fuzzy,
            content_for_replacement: match space.folded.into_inner() {
                Some(folded) if fuzzy => Cow::Owned(folded),
                _ => Cow::Borrowed(content),
            },
        },
    }
}

/// An edit with LF operands and its normalized needle.
struct Prepared {
    /// Needle with LF endings.
    old: String,
    /// Its fuzzy normalization.
    folded_old: String,
    /// Replacement with LF endings.
    new: String,
}

/// A validated match awaiting replacement.
struct Matched<'a> {
    /// Position in the supplied edit list.
    edit: usize,
    /// Byte offset in the base.
    index: usize,
    /// Byte length in the base.
    length: usize,
    /// Replacement text.
    new: &'a str,
}

/// Failure when `old` is not in the base.
fn not_found(path: &str, edit: usize, total: usize) -> DiagnosticErrorInfo {
    failure(&if total == 1 {
        format!(
            "Could not find the exact text in {path}. The old text must match exactly including all whitespace and newlines."
        )
    } else {
        format!(
            "Could not find edits[{edit}] in {path}. The oldText must match exactly including all whitespace and newlines."
        )
    })
}

/// Failure when `old` occurs more than once.
fn duplicate(path: &str, edit: usize, total: usize, count: usize) -> DiagnosticErrorInfo {
    failure(&if total == 1 {
        format!(
            "Found {count} occurrences of the text in {path}. The text must be unique. Please provide more context to make it unique."
        )
    } else {
        format!(
            "Found {count} occurrences of edits[{edit}] in {path}. Each oldText must be unique. Please provide more context to make it unique."
        )
    })
}

/// Failure when `old` is empty.
fn empty_old_text(path: &str, edit: usize, total: usize) -> DiagnosticErrorInfo {
    failure(&if total == 1 {
        format!("oldText must not be empty in {path}.")
    } else {
        format!("edits[{edit}].oldText must not be empty in {path}.")
    })
}

/// Failure when the result equals the base.
fn no_change(path: &str, total: usize) -> DiagnosticErrorInfo {
    failure(&if total == 1 {
        format!(
            "No changes made to {path}. The replacement produced identical content. This might indicate an issue with special characters or the text not existing as expected."
        )
    } else {
        format!("No changes made to {path}. The replacements produced identical content.")
    })
}

/// Match every edit against one base and check uniqueness.
fn locate<'a>(
    base: &Space,
    edits: &'a [Prepared],
    path: &str,
) -> Result<Vec<Matched<'a>>, DiagnosticErrorInfo> {
    let mut matched = Vec::with_capacity(edits.len());
    for (edit, prepared) in edits.iter().enumerate() {
        let hit = base
            .find(&prepared.old, &prepared.folded_old)
            .ok_or_else(|| not_found(path, edit, edits.len()))?;
        let count = base.occurrences(&prepared.old, &prepared.folded_old);
        if count > 1 {
            return Err(duplicate(path, edit, edits.len(), count));
        }
        matched.push(Matched {
            edit,
            index: hit.index,
            length: hit.length,
            new: &prepared.new,
        });
    }
    matched.sort_by_key(|entry| entry.index);
    for pair in matched.windows(2) {
        if pair[0].index + pair[0].length > pair[1].index {
            return Err(failure(&format!(
                "edits[{}] and edits[{}] overlap in {path}. Merge them into one edit or target disjoint regions.",
                pair[0].edit, pair[1].edit
            )));
        }
    }
    Ok(matched)
}

/// Apply edits to LF-normalized content, all matched against the same original snapshot.
///
/// `normalized_content` must already use LF endings; it is not converted here. If any
/// edit needs fuzzy matching, every edit is matched against the fuzzy-normalized content,
/// which is returned as the base. Each `new_text` is converted to LF, then inserted as is.
///
/// # Errors
/// Returns a diagnostic for an empty needle, a missing or repeated needle, overlapping
/// matches, or a result identical to the base.
pub fn apply_edits_to_normalized_content(
    normalized_content: &str,
    edits: &[Edit],
    path: &str,
) -> Result<AppliedEditsResult, DiagnosticErrorInfo> {
    let prepared: Vec<Prepared> = edits
        .iter()
        .map(|edit| {
            let old = normalize_to_lf(&edit.old_text);
            Prepared {
                folded_old: normalize_for_fuzzy_match(&old),
                old,
                new: normalize_to_lf(&edit.new_text),
            }
        })
        .collect();
    if let Some(empty) = prepared.iter().position(|edit| edit.old.is_empty()) {
        return Err(empty_old_text(path, empty, prepared.len()));
    }
    let original = Space::new(normalized_content);
    let fuzzy = prepared.iter().any(|edit| {
        original
            .find(&edit.old, &edit.folded_old)
            .is_some_and(|hit| hit.fuzzy)
    });
    let base_content = if fuzzy {
        original.folded().to_owned()
    } else {
        normalized_content.to_owned()
    };
    let matched = locate(&Space::new(&base_content), &prepared, path)?;
    let mut new_content = String::with_capacity(base_content.len());
    let mut cursor = 0;
    for entry in &matched {
        new_content.push_str(&base_content[cursor..entry.index]);
        new_content.push_str(entry.new);
        cursor = entry.index + entry.length;
    }
    new_content.push_str(&base_content[cursor..]);
    if new_content == base_content {
        return Err(no_change(path, prepared.len()));
    }
    Ok(AppliedEditsResult {
        base_content,
        new_content,
    })
}

#[cfg(test)]
mod tests;
