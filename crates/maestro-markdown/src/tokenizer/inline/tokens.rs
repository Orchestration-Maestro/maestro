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

/// A formatted span whose body is interpreted before the span itself is settled.
pub(super) struct Span {
    /// Authored length of the whole span.
    pub(super) size: usize,
    /// Body extent between the delimiters.
    pub(super) body: Range<usize>,
    /// Kind that wraps the interpreted body.
    pub(super) wrap: fn(Vec<Node>) -> Kind,
}

/// What the built-in grammar recognized at one cursor.
enum Built {
    /// A token that needs no further interpretation.
    Leaf(usize, Kind),
    /// A span whose body still has to be interpreted.
    Span(Span),
}

/// Where a token starts: the window limit and the scalar before it.
#[derive(Clone, Copy)]
struct Reached {
    /// Source byte a token must not extend past unless it is a formatted span.
    limit: usize,
    /// Previous scalar retained across plain text.
    previous: Option<char>,
}

/// A span waiting for its body.
struct Open {
    /// The recognized span.
    span: Span,
    /// Where the span started.
    reached: Reached,
}

/// One selected inline window and the span, if any, whose body is interpreted next.
struct Frame<'a> {
    /// Source extent of this window.
    extent: Range<usize>,
    /// Typed atom ranges shared by every window of one invocation.
    atoms: &'a [Range<usize>],
    /// Cursor and plain-text bookkeeping.
    state: InlineState,
    /// Nodes produced so far.
    output: Vec<Node>,
    /// Masked copy of this window.
    masked: String,
    /// Span whose body runs in the frame above this one.
    open: Option<Open>,
}

impl Tokenizer<'_> {
    /// Interpret one inline span without admitting block constructs.
    ///
    /// Nested spans run from an explicit stack of windows, so nesting depth never
    /// reaches the native stack.
    pub(crate) fn tokens(
        &mut self,
        extent: Range<usize>,
        atoms: &[Range<usize>],
        mut admission: Option<&mut crate::inline::InlineAdmission<'_>>,
    ) -> Vec<Node> {
        let mut parents = Vec::new();
        let mut current = self.frame(extent, atoms);
        loop {
            if let Some(body) = self.advance(&mut current, admission.as_deref_mut()) {
                let child = self.frame(body, atoms);
                parents.push(std::mem::replace(&mut current, child));
            } else if let Some(parent) = parents.pop() {
                let finished = std::mem::replace(&mut current, parent);
                self.close(&mut current, finished.output, admission.as_deref());
            } else {
                return current.output;
            }
        }
    }

    /// Open one window over `extent`, masking it once.
    fn frame<'a>(&self, extent: Range<usize>, atoms: &'a [Range<usize>]) -> Frame<'a> {
        #[cfg(test)]
        if self.scopes.is_none() {
            crate::tests::WORK.with_borrow_mut(|work| work.local_bytes += extent.len());
        }
        Frame {
            state: InlineState {
                cursor: extent.start,
                ..InlineState::default()
            },
            output: Vec::new(),
            masked: self.mask(extent.clone(), atoms),
            extent,
            atoms,
            open: None,
        }
    }

    /// Consume the window up to its end, or up to the body of the next formatted span.
    fn advance(
        &mut self,
        frame: &mut Frame<'_>,
        mut admission: Option<&mut crate::inline::InlineAdmission<'_>>,
    ) -> Option<Range<usize>> {
        let (extent, atoms) = (frame.extent.clone(), frame.atoms);
        while frame.state.cursor < extent.end {
            let cursor = frame.state.cursor;
            let previous = frame.state.kept.take();
            if let Some((size, index)) =
                self.caller_token(cursor..extent.end, admission.as_deref_mut())
            {
                frame
                    .output
                    .push(node(self.source, cursor..cursor + size, Kind::Atom(index)));
                frame.state.cursor += size;
                continue;
            }
            let atom = atoms.partition_point(|range| range.start < cursor);
            if let Some(range) = atoms.get(atom).filter(|range| range.start == cursor) {
                self.push_atom(range.clone(), atom, &mut frame.output);
                frame.state.cursor = range.end;
                continue;
            }
            let limit = atoms
                .get(atom)
                .map_or(extent.end, |range| range.start.min(extent.end));
            let reached = Reached { limit, previous };
            let masked = (frame.masked.as_str(), extent.start, previous);
            let built = self.builtin(cursor..extent.end, masked, atoms, admission.as_deref_mut());
            match built {
                Some(Built::Span(span)) => {
                    let body = span.body.clone();
                    frame.open = Some(Open { span, reached });
                    return Some(body);
                }
                Some(Built::Leaf(size, kind)) => {
                    self.settle(frame, reached, Some((size, kind)), admission.as_deref());
                }
                None => self.settle(frame, reached, None, admission.as_deref()),
            }
        }
        None
    }

    /// Wrap a finished body in its span and settle that span in the window that opened it.
    fn close(
        &mut self,
        parent: &mut Frame<'_>,
        children: Vec<Node>,
        admission: Option<&crate::inline::InlineAdmission<'_>>,
    ) {
        let Some(Open { span, reached }) = parent.open.take() else {
            return;
        };
        let kind = (span.wrap)(children);
        self.settle(parent, reached, Some((span.size, kind)), admission);
    }

    /// Keep a token that fits before the next atom, or consume fallback text instead.
    fn settle(
        &mut self,
        frame: &mut Frame<'_>,
        reached: Reached,
        token: Option<(usize, Kind)>,
        admission: Option<&crate::inline::InlineAdmission<'_>>,
    ) {
        let cursor = frame.state.cursor;
        let token = token.filter(|(size, kind)| {
            *size > 0 && fits(frame.atoms, cursor..cursor + size, reached.limit, kind)
        });
        if let Some((size, kind)) = token {
            self.context.track_anchor(&kind);
            frame
                .output
                .push(node(self.source, cursor..cursor + size, kind));
            frame.state.cursor += size;
            return;
        }
        self.fallback(
            (cursor..frame.extent.end, reached.limit, reached.previous),
            &mut frame.state,
            &mut frame.output,
            admission,
        );
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
        admission: Option<&mut crate::inline::InlineAdmission<'_>>,
    ) -> Option<Built> {
        let text = &self.source.text[range.clone()];
        let leaf = |(size, kind)| Built::Leaf(size, kind);
        self.admit_operand(range.clone(), atoms, admission)
            .map(leaf)
            .or_else(|| self.emphasis(range.clone(), masked).map(Built::Span))
            .or_else(|| self.line_break(text).map(leaf))
            .or_else(|| self.deletion(range).map(Built::Span))
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
