//! Cursor-time inline grammar over selected source windows.
use super::{Tokenizer, fits};
use crate::{Kind, Node, tokenizer::node};
use std::ops::Range;

/// Cursor and plain-text bookkeeping for one selected inline window.
#[derive(Default)]
struct InlineState {
    /// Next source byte to consume.
    cursor: usize,
    /// End of the last failed email-local run in this inline window.
    email_skip: usize,
    /// Previous scalar retained only across plain text.
    kept: Option<char>,
    /// Last plain-text node eligible for extension.
    plain_tail: Option<usize>,
}

impl Tokenizer<'_> {
    /// Interpret one inline span without admitting block constructs.
    pub(crate) fn tokens(
        &mut self,
        extent: Range<usize>,
        atoms: &[Range<usize>],
        mut admission: Option<&mut crate::inline::InlineAdmission<'_>>,
    ) -> Vec<Node> {
        #[cfg(test)]
        if self.scopes.is_none() {
            crate::tests::WORK.with_borrow_mut(|work| work.local_bytes += extent.len());
        }
        let mut output = Vec::new();
        let mut state = InlineState {
            cursor: extent.start,
            ..InlineState::default()
        };
        let masked = self.mask(extent.clone(), atoms);
        while state.cursor < extent.end {
            let previous = state.kept.take();
            if let Some((size, index)) =
                self.caller_token(state.cursor..extent.end, admission.as_deref_mut())
            {
                output.push(node(
                    self.source,
                    state.cursor..state.cursor + size,
                    Kind::Atom(index),
                ));
                state.cursor += size;
                continue;
            }
            let atom = atoms.partition_point(|range| range.start < state.cursor);
            if let Some(range) = atoms.get(atom).filter(|range| range.start == state.cursor) {
                self.push_atom(range.clone(), atom, &mut output);
                state.cursor = range.end;
                continue;
            }
            let limit = atoms
                .get(atom)
                .map_or(extent.end, |range| range.start.min(extent.end));
            let token = self.builtin(
                state.cursor..extent.end,
                (masked.as_str(), extent.start, previous),
                atoms,
                admission.as_deref_mut(),
            );
            if let Some((size, kind)) = token.filter(|(size, kind)| {
                *size > 0 && fits(atoms, state.cursor..state.cursor + size, limit, kind)
            }) {
                self.context.track_anchor(&kind);
                output.push(node(self.source, state.cursor..state.cursor + size, kind));
                state.cursor += size;
                continue;
            }
            self.fallback(
                (state.cursor..extent.end, limit, previous),
                &mut state,
                &mut output,
                admission.as_deref(),
            );
        }
        output
    }

    /// Emit a reached bare address or consume one caller-selected fallback token.
    fn fallback(
        &self,
        (range, limit, previous): (Range<usize>, usize, Option<char>),
        state: &mut InlineState,
        output: &mut Vec<Node>,
        admission: Option<&crate::inline::InlineAdmission<'_>>,
    ) {
        let text = &self.source.text[range.clone()];
        let cursor = state.cursor;
        let address = self.address(range.clone(), &mut state.email_skip);
        if let Some((size, href)) = address.filter(|(size, _)| cursor + size <= limit) {
            if self.scopes.is_none() {
                self.push_text(cursor..cursor + size, output, &mut state.plain_tail);
                state.cursor += size;
                return;
            }
            let label = text[..size].to_owned();
            let child = node(
                self.source,
                cursor..cursor + size,
                Kind::Text(crate::Text {
                    decoded: label.clone(),
                    authored: label.clone(),
                }),
            );
            output.push(node(
                self.source,
                cursor..cursor + size,
                Kind::Link(
                    vec![child],
                    label,
                    href,
                    crate::LinkMetadata {
                        title: String::new(),
                        link_type: crate::LinkType::Autolink,
                    },
                ),
            ));
            state.cursor += size;
            return;
        }
        let hint = self.caller_hint(range, admission);
        let line = &text[..limit.min(hint) - cursor];
        let size = self.text(line);
        self.push_text(cursor..cursor + size, output, &mut state.plain_tail);
        state.kept = line[..size]
            .chars()
            .next_back()
            .filter(|&c| c != '_')
            .or(previous);
        state.cursor += size;
    }

    /// Try built-in grammar with the same selected recursive admission window.
    fn builtin(
        &mut self,
        range: Range<usize>,
        masked: (&str, usize, Option<char>),
        atoms: &[Range<usize>],
        mut admission: Option<&mut crate::inline::InlineAdmission<'_>>,
    ) -> Option<(usize, Kind)> {
        let text = &self.source.text[range.clone()];
        self.admit_operand(range.clone(), atoms, admission.as_deref_mut())
            .or_else(|| self.emphasis(range.clone(), masked, atoms, admission.as_deref_mut()))
            .or_else(|| self.line_break(text))
            .or_else(|| self.deletion(range, atoms, admission))
    }

    /// Clip fallback text at a valid caller hint after its first scalar.
    fn caller_hint(
        &self,
        range: Range<usize>,
        admission: Option<&crate::inline::InlineAdmission<'_>>,
    ) -> usize {
        let text = &self.source.text[range.clone()];
        let first = text.chars().next().map_or(0, char::len_utf8);
        let suffix = &text[first..];
        admission
            .and_then(|admission| admission.tokenizer.start(suffix))
            .filter(|&offset| suffix.is_char_boundary(offset))
            .map_or(range.end, |offset| range.start + first + offset)
    }

    /// Try caller grammar before built-in grammar without hiding its delimiters.
    fn caller_token(
        &self,
        range: Range<usize>,
        admission: Option<&mut crate::inline::InlineAdmission<'_>>,
    ) -> Option<(usize, usize)> {
        let admission = admission?;
        let token = admission
            .tokenizer
            .tokenize(&self.source.text[range.clone()])?;
        let end = range.start.checked_add(token.length)?;
        if token.length == 0 || end > range.end || !self.source.text.is_char_boundary(end) {
            return None;
        }
        let native = self.source.extent(range.start..end);
        let index = admission
            .barriers
            .partition_point(|barrier| barrier.end <= native.start);
        if admission
            .barriers
            .get(index)
            .is_some_and(|barrier| barrier.start < native.end)
        {
            return None;
        }
        let index = admission.offset + admission.ids.len();
        admission.ids.push(token.index);
        Some((token.length, index))
    }
}
