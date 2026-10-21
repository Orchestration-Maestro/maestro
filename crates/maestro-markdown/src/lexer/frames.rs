//! Quote chunk suspension and container-tail resumption.
use super::{Lexer, blocks::BlockTokens, quote::quote_source};
use crate::{
    Kind, Node, rules,
    source::{Cursor, ScopeSource, Segmented, Source},
};

/// Retained quote invocation state across nested block admission.
struct QuoteFrame {
    /// Original logical operand, shared with all selections and published raw text.
    source: Segmented,
    /// Unentered source, without a joined remainder string.
    pending: Cursor,
    /// Published semantic body with its native provenance.
    text: Segmented,
    /// Ordered children and the effective grammar tail.
    output: BlockTokens,
    /// Furthest logical byte consumed, independent of reentry truncation.
    consumed: usize,
    /// Whether the preceding raw line admits lazy paragraph continuation.
    lazy: bool,
    /// A resumed quote or terminal code completes its enclosing invocation.
    finished: bool,
    /// A trimmed list terminator is a retained separator, not a new admission boundary.
    list_separator: bool,
}

/// Next action after admitting one quote chunk.
enum Resume {
    /// Keep consuming the current quote's input.
    Continue,
    /// Suspend it while a nested quote consumes its own raw producer.
    Quote(Segmented),
}

impl Resume {
    /// Transfer suspended input to its next quote invocation.
    fn frame(self) -> Option<QuoteFrame> {
        match self {
            Self::Continue => None,
            Self::Quote(source) => Some(QuoteFrame::new(source)),
        }
    }
}

impl<E> Lexer<'_, E> {
    /// Bound initial quote admission in its existing contiguous caller operand.
    pub(super) fn quote(&mut self, source: &Source<'_>) -> Option<(usize, Node)> {
        let first = source.text.split_inclusive('\n').next()?;
        if crate::source::strip_quote(first).len() == first.len() {
            return None;
        }
        #[cfg(test)]
        crate::tests::block_operand(source.text);
        let matched = rules::block_blockquote(self.options)?.find(source.text)?;
        let limit = source.text[matched.range()].trim_end_matches('\n').len();
        let (raw, node, _) = self.quote_frame(source.select(std::iter::once(0..limit)).into())?;
        let consumed = source.local(&raw.native(&(0..raw.len()))?)?.end;
        Some((consumed, node))
    }

    /// Drive quote suspension with an explicit stack of invocation-local frames.
    fn quote_frame(&mut self, source: Segmented) -> Option<(Segmented, Node, Segmented)> {
        let mut frames = vec![QuoteFrame::new(source)];
        loop {
            let frame = frames.last_mut()?;
            if !frame.finished
                && let Some(current) = frame.next(self.options)
            {
                self.quote_chunk(frame, &current)?;
                let resumed = self.resume_container(frame)?;
                frames.extend(resumed.frame());
                continue;
            }
            let frame = frames.pop()?;
            if frame.consumed == 0 {
                return None;
            }
            let raw = frame.source.select(0..frame.consumed);
            let node = Node {
                authored: raw.mapped(),
                range: raw.native(&(0..raw.len()))?,
                kind: Kind::Quote(frame.output.nodes, frame.text.mapped()),
            };
            let rest = frame.source.select(frame.consumed..frame.source.len());
            let Some(parent) = frames.last_mut() else {
                return Some((raw, node, rest));
            };
            Self::publish_resume(parent, raw, node, rest)?;
        }
    }

    /// Enter one transformed chunk without changing its enclosing item grammar state.
    fn quote_chunk(&mut self, frame: &mut QuoteFrame, current: &ScopeSource) -> Option<()> {
        let end = frame
            .source
            .local(&current.extent(0..current.text.len()))?
            .end;
        frame.consumed = frame.consumed.max(end);
        let chunk = quote_source(current);
        if frame.text.len() != 0 {
            frame.text.newline();
        }
        let top = self.top;
        self.top = true;
        self.block_tokens(&chunk, &mut frame.output, true);
        self.top = top;
        frame.text.append(chunk.into());
        frame.finished = frame.pending.is_empty()
            || frame
                .output
                .last()
                .is_some_and(|node| matches!(node.kind, Kind::CodeBlock(..)));
        Some(())
    }

    /// Suspend the enclosing quote before entering its trailing container's raw producer.
    fn resume_container(&mut self, frame: &mut QuoteFrame) -> Option<Resume> {
        if frame.finished
            || !frame
                .output
                .last()
                .is_some_and(|node| matches!(node.kind, Kind::List(..) | Kind::Quote(..)))
        {
            return Some(Resume::Continue);
        }
        let old = frame.output.nodes.pop()?;
        let mut joined = frame.output.raw()?;
        let prefix = match &old.kind {
            Kind::Quote(_, body) => frame.text.len().checked_sub(body.len())?,
            _ => frame.text.local(&joined.native(&(0..joined.len()))?)?.start,
        };
        joined.newline();
        joined.append(std::mem::take(&mut frame.pending).seal());
        self.forget(&old);
        frame.text.truncate(prefix);
        if matches!(old.kind, Kind::Quote(..)) {
            frame.finished = true;
            return Some(Resume::Quote(joined));
        }
        let operand = joined.materialize();
        let (consumed, node) = self.list(&operand.view(0..operand.text.len()))?;
        Self::publish_resume(
            frame,
            joined.select(0..consumed),
            node,
            joined.select(consumed..joined.len()),
        )?;
        Some(Resume::Continue)
    }

    /// Publish a resumed container while preserving its whole-body atomic projection.
    fn publish_resume(
        frame: &mut QuoteFrame,
        used: Segmented,
        node: Node,
        rest: Segmented,
    ) -> Option<()> {
        let extent = used.native(&(0..used.len()))?;
        let end = frame.source.local(&extent)?.end;
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| {
            work.reentry_end = end;
            work.reentry_previous = frame.consumed;
        });
        frame.consumed = frame.consumed.max(end);
        if let Kind::Quote(_, body) = &node.kind {
            frame.text.append(Segmented::spelling(body.clone(), extent));
        } else {
            frame.text.append(used.clone());
        }
        frame.output.push_segmented(used, node);
        frame.pending.segmented(rest);
        frame.list_separator = true;
        Some(())
    }

    /// Release inline scopes of replaced containers before callback replay admission.
    fn forget(&mut self, node: &Node) {
        match &node.kind {
            Kind::Paragraph(_) | Kind::Inline(_) | Kind::Heading(..) => {
                self.scopes
                    .remove(&(node.range.start, node.range.end, false));
            }
            Kind::Quote(children, _) | Kind::List(_, children) | Kind::Item(children) => {
                for child in children {
                    self.forget(child);
                }
            }
            Kind::Table(table) => {
                for cell in table.header.iter().chain(table.rows.iter().flatten()) {
                    self.scopes
                        .remove(&(cell.range.start, cell.range.end, false));
                }
            }
            _ => {}
        }
    }
}

impl QuoteFrame {
    /// Retain an operand and its invocation-local grammar output.
    fn new(source: Segmented) -> Self {
        let mut pending = Cursor::default();
        pending.segmented(source.clone());
        Self {
            text: ScopeSource::identity("").into(),
            source,
            pending,
            output: BlockTokens::default(),
            consumed: 0,
            lazy: false,
            finished: false,
            list_separator: false,
        }
    }

    /// Resume at the next lazy-plus-quoted run, preserving its following lookahead.
    fn next(&mut self, options: crate::MarkedOptions) -> Option<ScopeSource> {
        let mut chunk = Cursor::default();
        let mut quoted = false;
        while let Some(line) = self.pending.line() {
            let line = line.materialize();
            #[cfg(test)]
            crate::tests::WORK.with_borrow_mut(|work| work.quote_lines += 1);
            let marker = crate::source::strip_quote(&line.text).len() != line.text.len();
            let separator = self.list_separator && &*line.text == "\n";
            self.list_separator = separator;
            if quoted && !marker || !separator && !self.admits(&line.text, marker, options) {
                self.pending.prepend(line);
                break;
            }
            if marker {
                quoted = true;
                let text = line.text.trim_start_matches(' ').strip_prefix('>')?;
                self.lazy = !text
                    .strip_prefix(' ')
                    .unwrap_or(text)
                    .trim_end_matches('\n')
                    .is_empty();
            }
            let mut input = Cursor::default();
            input.append(line);
            chunk.extend(input);
        }
        if chunk.is_empty() {
            return None;
        }
        let chunk = chunk.materialize();
        let end = chunk.text.trim_end_matches('\n').len();
        (end > 0).then(|| chunk.select(std::iter::once(0..end)))
    }

    /// Use the existing quote grammar only on the line whose admission is undecided.
    fn admits(&self, line: &str, marker: bool, options: crate::MarkedOptions) -> bool {
        if marker {
            return true;
        }
        if !self.lazy {
            return false;
        }
        let operand = ["> x\n", line].concat();
        #[cfg(test)]
        crate::tests::block_operand(&operand);
        rules::block_blockquote(options)
            .and_then(|rule| rule.find(&operand))
            .is_some_and(|matched| matched.end() > 4)
    }
}
