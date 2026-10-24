//! Word tokens and their ordered changed values.

use super::{
    sequence::{Kind, diff_sequence},
    whitespace,
};
use maestro_text::is_whitespace_scalar as ws;

/// A joined token run with its edit disposition.
pub(super) struct Run {
    /// Edit disposition.
    pub kind: Kind,
    /// Joined text, adjusted at adjacent whitespace boundaries.
    pub value: String,
}

/// Membership in the word token alphabet.
fn word(scalar: char) -> bool {
    scalar.is_ascii_alphanumeric()
        || matches!(scalar,
        '_' | '\u{ad}' | '\u{c0}'..='\u{d6}' | '\u{d8}'..='\u{f6}' |
        '\u{f8}'..='\u{2c6}' | '\u{2c8}'..='\u{2d7}' | '\u{2de}'..='\u{2ff}' |
        '\u{1e00}'..='\u{1eff}')
}

/// Tokenize once, attaching whitespace to both adjacent nonwhitespace tokens.
fn tokens(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut chars = text.char_indices().peekable();
    let mut leading = 0;
    while let Some((at, scalar)) = chars.next() {
        if ws(scalar) {
            continue;
        }
        let start = leading;
        let mut end = at + scalar.len_utf8();
        if word(scalar) {
            while let Some((next, scalar)) = chars.next_if(|(_, scalar)| word(*scalar)) {
                end = next + scalar.len_utf8();
            }
        }
        leading = end;
        while let Some(&(next, scalar)) = chars.peek() {
            if !ws(scalar) {
                break;
            }
            chars.next();
            end = next + scalar.len_utf8();
        }
        out.push(&text[start..end]);
    }
    if out.is_empty() && !text.is_empty() {
        out.push(text);
    }
    out
}

/// Join consecutive tokens without repeating shared whitespace.
fn join(tokens: &[&str]) -> String {
    tokens
        .iter()
        .enumerate()
        .map(|(index, token)| {
            if index == 0 {
                *token
            } else {
                token.trim_start_matches(ws)
            }
        })
        .collect()
}

/// Compare trimmed token keys, retaining unchanged values from the new side.
pub(super) fn diff_words(old: &str, new: &str) -> Vec<Run> {
    let old = tokens(old);
    let new = tokens(new);
    let old_keys: Vec<_> = old.iter().map(|token| token.trim_matches(ws)).collect();
    let new_keys: Vec<_> = new.iter().map(|token| token.trim_matches(ws)).collect();
    let (mut old_at, mut new_at) = (0, 0);
    let mut runs: Vec<_> = diff_sequence(&old_keys, &new_keys)
        .into_iter()
        .map(|(kind, count)| {
            let value = match kind {
                Kind::Removed => {
                    let value = join(&old[old_at..old_at + count]);
                    old_at += count;
                    value
                }
                Kind::Added | Kind::Unchanged => {
                    let value = join(&new[new_at..new_at + count]);
                    new_at += count;
                    if kind == Kind::Unchanged {
                        old_at += count;
                    }
                    value
                }
            };
            Run { kind, value }
        })
        .collect();
    whitespace::reconcile(&mut runs);
    runs
}
