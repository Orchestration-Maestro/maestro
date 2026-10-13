//! Whitespace ownership at adjacent kept and changed word runs.

use super::{
    sequence::{Kind, prefix_lengths},
    word::Run,
};
use maestro_text::is_whitespace_scalar as ws;

/// Leading whitespace, borrowed from the value.
fn head(value: &str) -> &str {
    &value[..value.len() - value.trim_start_matches(ws).len()]
}
/// Trailing whitespace, borrowed from the value.
fn tail(value: &str) -> &str {
    &value[value.trim_end_matches(ws).len()..]
}
/// Byte length of the common scalar prefix.
fn prefix(left: &str, right: &str) -> usize {
    left.chars()
        .zip(right.chars())
        .take_while(|(a, b)| a == b)
        .map(|(a, _)| a.len_utf8())
        .sum()
}
/// Byte length of the common scalar suffix.
fn suffix(left: &str, right: &str) -> usize {
    left.chars()
        .rev()
        .zip(right.chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(a, _)| a.len_utf8())
        .sum()
}
/// Maximum suffix of `left` that is a prefix of `right`, using one linear pass.
fn overlap(left: &str, right: &str) -> usize {
    if right.is_empty() {
        return 0;
    }
    let right = right.as_bytes();
    let border = prefix_lengths(right);
    let mut matched = 0;
    for &byte in left.as_bytes() {
        while matched > 0 && (matched == right.len() || byte != right[matched]) {
            matched = border[matched - 1];
        }
        if byte == right[matched] {
            matched += 1;
        }
    }
    matched
}
/// Replace the prefix of byte length `length`.
fn replace_head(value: &mut String, length: usize, replacement: &str) {
    value.replace_range(..length, replacement);
}
/// Replace the suffix of byte length `length`.
fn replace_tail(value: &mut String, length: usize, replacement: &str) {
    value.replace_range(value.len() - length.., replacement);
}
/// Replacement runs share only whitespace common to both sides.
fn replacement(
    start: Option<&mut String>,
    deleted: &mut String,
    inserted: &mut String,
    end: Option<&mut String>,
) {
    let common_head = prefix(head(deleted), head(inserted));
    let common_tail = suffix(tail(deleted), tail(inserted));
    if let Some(start) = start {
        let length = head(inserted).len();
        replace_tail(start, length, &inserted[..common_head]);
        replace_head(deleted, common_head, "");
        replace_head(inserted, common_head, "");
    }
    if let Some(end) = end {
        let length = tail(inserted).len();
        replace_head(end, length, &inserted[inserted.len() - common_tail..]);
        replace_tail(deleted, common_tail, "");
        replace_tail(inserted, common_tail, "");
    }
}
/// A middle deletion apportions new whitespace to its two kept neighbours.
fn middle_deletion(start: &mut String, deleted: &mut String, end: &mut String) {
    let full = head(end);
    let leading = prefix(full, head(deleted));
    let trailing = suffix(&full[leading..], tail(deleted));
    replace_head(deleted, leading, "");
    replace_tail(deleted, trailing, "");
    replace_tail(start, full.len(), &full[..full.len() - trailing]);
    let length = full.len() - trailing;
    replace_head(end, length, "");
}
/// Remove only overlapping whitespace from a one-sided deletion.
fn deletion(start: Option<&mut String>, deleted: &mut String, end: Option<&mut String>) {
    match (start, end) {
        (Some(start), Some(end)) => middle_deletion(start, deleted, end),
        (None, Some(end)) => {
            let n = overlap(tail(deleted), head(end));
            replace_tail(deleted, n, "");
        }
        (Some(start), None) => {
            let n = overlap(tail(start), head(deleted));
            replace_head(deleted, n, "");
        }
        (None, None) => {}
    }
}
/// Assign whitespace for one group of edits between optional kept runs.
fn group(start: Option<&mut String>, changes: &mut [Run], end: Option<&mut String>) {
    let (mut removed, mut added) = (None, None);
    for run in changes {
        match run.kind {
            Kind::Removed => removed = Some(&mut run.value),
            Kind::Added => added = Some(&mut run.value),
            Kind::Unchanged => {}
        }
    }
    match (removed, added) {
        (Some(removed), Some(added)) => replacement(start, removed, added, end),
        (Some(removed), None) => deletion(start, removed, end),
        (None, Some(added)) => {
            if start.is_some() {
                replace_head(added, head(added).len(), "");
            }
            if let Some(end) = end {
                replace_head(end, head(end).len(), "");
            }
        }
        (None, None) => {}
    }
}
/// Traverse edit groups once, retaining independent mutable neighbour slots.
pub(super) fn reconcile(runs: &mut [Run]) {
    let mut at = 0;
    while at < runs.len() {
        if runs[at].kind == Kind::Unchanged {
            at += 1;
            continue;
        }
        let end = at
            + runs[at..]
                .iter()
                .take_while(|run| run.kind != Kind::Unchanged)
                .count();
        let (before, rest) = runs.split_at_mut(at);
        let (changes, after) = rest.split_at_mut(end - at);
        group(
            before.last_mut().map(|run| &mut run.value),
            changes,
            after.first_mut().map(|run| &mut run.value),
        );
        at = end;
    }
}
