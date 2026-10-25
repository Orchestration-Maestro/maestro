//! Suspended block admission, quote chunks and container resumption.
use super::{
    BlockRun, BlockTokens, Level, Lexer,
    blocks::Mark,
    quote::{QuoteEntry, quote_source},
};
use crate::{
    Kind, Node,
    inline::ScopeKey,
    rules,
    source::{Cursor, ScopeSource, Segmented},
    tokenizer::block::{ListEntry, ListFrame, ListState, ListTarget},
};
use std::collections::HashMap;

/// One admission level of the document.
pub(crate) enum Frame {
    /// The document level, whose completion needs no delivery.
    Document(BlockRun),
    /// A nested block admission level.
    Run(BlockRun),
    /// A quote level and its chunk loop.
    Quote(Box<Quote>),
    /// A list level and its item loop.
    List(Box<ListFrame>),
}

/// Result of one level step.
pub(crate) enum Step {
    /// This level continues with its next step.
    Again,
    /// This level is suspended under a new child level.
    Push(Frame),
    /// This level completed and delivers its result to the level below.
    Done(Done),
    /// The document level completed with its retained output.
    Finish(BlockTokens),
}

/// What a completed level delivers to the level that suspended for it.
pub(crate) enum Done {
    /// A block admission level produced its output and its open step.
    Blocks(BlockTokens, Option<(usize, Mark)>),
    /// A container step in a block level produced its node.
    Consumed {
        /// Logical bytes the container's own operand consumed.
        consumed: usize,
        /// The admitted block node.
        node: Node,
        /// Retained trailing list, present only for a list.
        list: Option<Box<ListState>>,
    },
    /// A container level resumed inside a quote produced its operands.
    Resumed {
        /// Operand the resumed container itself consumed.
        used: Segmented,
        /// The admitted block node.
        node: Node,
        /// Operand after the resumed container.
        rest: Segmented,
        /// Retained trailing list, present only for a list.
        list: Option<Box<ListState>>,
    },
    /// The container grammar declined its operand.
    Rejected,
}

/// A recognized container whose body is admitted by its own level.
pub(crate) enum Pending {
    /// A quote prefix recognized over a bounded operand.
    Quote(QuoteEntry),
    /// A list marker recognized over its first line.
    List(ListEntry),
}

/// Where a completed quote level delivers its node.
enum QuoteTarget {
    /// A container step in an enclosing block level, projected through its raw operand.
    Container(ScopeSource),
    /// The enclosing quote level, which publishes this node into its output.
    Nested,
}

/// Next action of one quote level.
enum QuotePhase {
    /// Admit the next quoted chunk.
    Chunk,
    /// Resume a container the quoted chunk ended with.
    Resume,
    /// Publish the completed level.
    Publish,
    /// This level cannot publish its own operand.
    Reject,
}

/// One quote level and its chunk loop.
pub(crate) struct Quote {
    /// Retained chunk invocation state.
    frame: QuoteFrame,
    /// Where this level delivers its node.
    target: QuoteTarget,
    /// Enclosing grammar position restored when this level completes.
    top: bool,
    /// Next action of this level.
    phase: QuotePhase,
}

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

/// Next action after admitting one quoted chunk.
enum Resume {
    /// Keep consuming the current quote's input.
    Continue,
    /// Suspend it while a nested quote consumes its own raw producer.
    Quote(Segmented),
    /// Admit the resumed trailing list over the joined raw producer.
    List {
        /// Joined raw producer of the resumed list.
        joined: Segmented,
        /// Retained list identity and its completed items.
        state: Box<ListState>,
        /// Item wrappers already published for the completed items.
        items: Vec<Node>,
    },
}

/// What one chunk step must do next.
enum Next {
    /// Admit this transformed chunk through a block level.
    Run(Segmented),
    /// The level has no further chunk.
    Publish,
    /// The level cannot project its own operand.
    Rejected,
}

impl Frame {
    /// Advance one admission level.
    fn step<E>(&mut self, lexer: &mut Lexer<'_, E>) -> Step {
        match self {
            Self::Document(run) => match run.step(lexer) {
                Step::Done(Done::Blocks(output, _)) => Step::Finish(output),
                step => step,
            },
            Self::Run(run) => run.step(lexer),
            Self::Quote(quote) => quote.step(lexer),
            Self::List(list) => list.step(lexer),
        }
    }

    /// Continue after the level this frame suspended for completed.
    fn resumed<E>(&mut self, done: Done, lexer: &mut Lexer<'_, E>) -> Option<Frame> {
        match self {
            Self::Document(run) | Self::Run(run) => run.resumed(lexer, done),
            Self::Quote(quote) => quote.resumed(lexer, done),
            Self::List(list) => list.resumed(lexer, done),
        }
    }
}

impl Quote {
    /// Start one container quote level over its recognized prefix.
    fn start(entry: QuoteEntry, top: bool) -> Self {
        Self {
            frame: QuoteFrame::new(entry.body),
            target: QuoteTarget::Container(entry.raw),
            top,
            phase: QuotePhase::Chunk,
        }
    }

    /// Start one quote level that feeds the enclosing quote.
    fn nested(source: Segmented, top: bool) -> Self {
        Self {
            frame: QuoteFrame::new(source),
            target: QuoteTarget::Nested,
            top,
            phase: QuotePhase::Chunk,
        }
    }

    /// Admit the next chunk or publish this level.
    fn step<E>(&mut self, lexer: &mut Lexer<'_, E>) -> Step {
        match self.phase {
            QuotePhase::Chunk => match self.chunk(lexer) {
                Next::Run(chunk) => {
                    let output = std::mem::take(&mut self.frame.output);
                    self.top = lexer.top;
                    lexer.top = true;
                    let level = Level::Chunk;
                    Step::Push(Frame::Run(lexer.block_run(chunk, output, level)))
                }
                Next::Publish => {
                    self.phase = QuotePhase::Publish;
                    Step::Again
                }
                Next::Rejected => self.rejected(lexer),
            },
            QuotePhase::Resume => match lexer.resume_container(&mut self.frame) {
                None => self.rejected(lexer),
                Some(Resume::Continue) => {
                    self.phase = QuotePhase::Chunk;
                    Step::Again
                }
                Some(Resume::Quote(source)) => {
                    let top = lexer.top;
                    Step::Push(Frame::Quote(Box::new(Self::nested(source, top))))
                }
                Some(Resume::List {
                    joined,
                    state,
                    items,
                }) => {
                    let list = ListFrame::resuming(joined, state, items, ListTarget::Resumed);
                    Step::Push(Frame::List(Box::new(list)))
                }
            },
            QuotePhase::Publish => self.publish(lexer),
            QuotePhase::Reject => Step::Done(Done::Rejected),
        }
    }

    /// Select the next chunk, or report that this level has none left.
    fn chunk<E>(&mut self, lexer: &Lexer<'_, E>) -> Next {
        if self.frame.finished {
            return Next::Publish;
        }
        let Some(current) = self.frame.next(lexer.options) else {
            return Next::Publish;
        };
        let Some(extent) = self
            .frame
            .source
            .local(&current.extent(0..current.text.len()))
        else {
            return Next::Rejected;
        };
        self.frame.consumed = self.frame.consumed.max(extent.end);
        let chunk: Segmented = quote_source(&current).into();
        if self.frame.text.len() != 0 {
            self.frame.text.newline();
        }
        self.frame.text.append(&chunk);
        Next::Run(chunk)
    }

    /// Continue after the level this frame suspended for completed.
    fn resumed<E>(&mut self, lexer: &mut Lexer<'_, E>, done: Done) -> Option<Frame> {
        match done {
            Done::Blocks(output, _) => {
                self.frame.output = output;
                self.frame.finished = self.frame.pending.is_empty()
                    || self
                        .frame
                        .output
                        .last()
                        .is_some_and(|node| matches!(node.kind, Kind::CodeBlock(..)));
                lexer.top = self.top;
                self.phase = QuotePhase::Resume;
            }
            Done::Resumed {
                used,
                node,
                rest,
                list,
            } => {
                publish_resume(&mut self.frame, used, node, rest, list)?;
                self.phase = QuotePhase::Chunk;
            }
            Done::Rejected => self.phase = QuotePhase::Reject,
            Done::Consumed { .. } => {}
        }
        None
    }

    /// Deliver the completed level to the level that suspended for it.
    fn publish<E>(&mut self, lexer: &mut Lexer<'_, E>) -> Step {
        lexer.top = self.top;
        let Some(node) = self.admitted(lexer) else {
            return self.rejected(lexer);
        };
        match &self.target {
            QuoteTarget::Container(raw) => match raw.local(&node.range) {
                Some(consumed) => Step::Done(Done::Consumed {
                    consumed: consumed.end,
                    node,
                    list: None,
                }),
                None => self.rejected(lexer),
            },
            QuoteTarget::Nested => {
                let consumed = self.frame.consumed;
                Step::Done(Done::Resumed {
                    used: self.frame.source.select(0..consumed),
                    node,
                    rest: self.frame.source.select(consumed..self.frame.source.len()),
                    list: None,
                })
            }
        }
    }

    /// Build the admitted node, or `None` when its extent cannot be projected.
    fn admitted<E>(&mut self, lexer: &mut Lexer<'_, E>) -> Option<Node> {
        let consumed = self.frame.consumed;
        if consumed == 0 {
            return None;
        }
        let raw = self.frame.source.select(0..consumed);
        let range = raw.native(&(0..raw.len()))?;
        let children = lexer.finish_blocks(std::mem::take(&mut self.frame.output));
        Some(Node {
            authored: raw.mapped(),
            range,
            kind: Kind::Quote(children, self.frame.text.mapped()),
        })
    }

    /// Report that this level admitted nothing.
    fn rejected<E>(&mut self, lexer: &mut Lexer<'_, E>) -> Step {
        lexer.top = self.top;
        Step::Done(Done::Rejected)
    }
}

impl<E> Lexer<'_, E> {
    /// Suspend the current level under a recognized container's own level.
    pub(super) fn admit(&mut self, pending: Pending) -> Frame {
        match pending {
            Pending::Quote(entry) => {
                let top = self.top;
                Frame::Quote(Box::new(Quote::start(entry, top)))
            }
            Pending::List(entry) => {
                Frame::List(Box::new(ListFrame::new(entry, ListTarget::Container)))
            }
        }
    }

    /// Admit blocks through one explicit frame stack instead of the caller's stack.
    ///
    /// The stack empties only through [`Step::Finish`], which returns before the next step.
    pub(super) fn drive(&mut self, root: Frame) -> BlockTokens {
        let mut frames = vec![root];
        loop {
            let step = match frames.last_mut() {
                Some(frame) => frame.step(self),
                None => return BlockTokens::default(),
            };
            match step {
                Step::Again => {}
                Step::Push(frame) => frames.push(frame),
                Step::Finish(output) => return output,
                Step::Done(done) => deliver(&mut frames, done, self),
            }
        }
    }

    /// Bound initial quote admission in its existing contiguous caller operand.
    pub(super) fn quote_entry(&mut self, source: &Segmented) -> Option<QuoteEntry> {
        let first = source.first_line();
        if crate::source::strip_quote(&first.text).len() == first.text.len() {
            return None;
        }
        let (raw, matched) = quote_prefix(source, rules::block_blockquote(self.options)?)?;
        self.undecided |= !source.settled_after(matched.end());
        let view = raw.view(0..raw.text.len());
        let limit = view.text[matched.range()].trim_end_matches('\n').len();
        let body = view.select(std::iter::once(0..limit)).into();
        Some(QuoteEntry { raw, body })
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
        let mut old = frame.output.nodes.pop()?;
        let mut joined = frame.output.raw()?;
        // A container whose own body is this text's tail loses exactly that body. One a
        // nested level published spells the same content with its quoting removed, so only
        // the container's authored start names a position this text owns.
        let authored = frame.text.local(&joined.native(&(0..joined.len()))?)?.start;
        let prefix = match &old.kind {
            Kind::Quote(_, body) if frame.text.ends_with(body) => frame.text.len() - body.len(),
            _ => authored,
        };
        joined.newline();
        joined.append(&std::mem::take(&mut frame.pending).seal());
        forget(&mut self.scopes, &old);
        frame.text.truncate(prefix);
        if matches!(old.kind, Kind::Quote(..)) {
            frame.finished = true;
            return Some(Resume::Quote(joined));
        }
        let Kind::List(_, items) = std::mem::replace(&mut old.kind, Kind::Gap) else {
            return None;
        };
        let state = frame.output.take_list()?;
        Some(Resume::List {
            joined,
            state,
            items,
        })
    }
}

/// Publish a resumed container while preserving its whole-body atomic projection.
fn publish_resume(
    frame: &mut QuoteFrame,
    used: Segmented,
    node: Node,
    rest: Segmented,
    list: Option<Box<ListState>>,
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
        frame
            .text
            .append(&Segmented::spelling(body.clone(), extent));
    } else {
        frame.text.append(&used);
    }
    frame.output.push_container(used, node, list);
    frame.pending.segmented(rest);
    frame.list_separator = true;
    Some(())
}

/// Hand one completed level's result to the level that suspended for it.
fn deliver<E>(frames: &mut Vec<Frame>, done: Done, lexer: &mut Lexer<'_, E>) {
    frames.pop();
    let resumed = frames
        .last_mut()
        .and_then(|parent| parent.resumed(done, lexer));
    if let Some(frame) = resumed {
        frames.push(frame);
    }
}

/// Materialize line-bounded prefixes whose length doubles until a complete line follows
/// the grammar's match or the operand ends; there the match equals the whole operand's.
fn quote_prefix(
    source: &Segmented,
    rule: &regress::Regex,
) -> Option<(ScopeSource, regress::Match)> {
    let mut lines = rules::lines::Cursor::default();
    let mut end = 0;
    loop {
        let target = (2 * end).max(1);
        while end < target {
            end = lines.next(source).map_or(source.len(), |line| line.end);
            if end == source.len() {
                break;
            }
        }
        let raw = source.select(0..end).materialize();
        #[cfg(test)]
        crate::tests::block_operand(&raw.text);
        let matched = rule.find(&raw.text)?;
        if end == source.len() || raw.text[matched.end()..].contains('\n') {
            return Some((raw, matched));
        }
    }
}

/// Release the inline scopes of a replaced or re-admitted node and its descendants.
pub(super) fn forget<T>(scopes: &mut HashMap<ScopeKey, T>, node: &Node) {
    let mut pending = vec![node];
    while let Some(node) = pending.pop() {
        match &node.kind {
            Kind::Paragraph(_) | Kind::Inline(_) | Kind::Heading(..) => {
                scopes.remove(&(node.range.start, node.range.end, false));
            }
            Kind::Quote(children, _) | Kind::List(_, children) | Kind::Item(children) => {
                pending.extend(children);
            }
            Kind::Table(table) => {
                for cell in table.header.iter().chain(table.rows.iter().flatten()) {
                    scopes.remove(&(cell.range.start, cell.range.end, false));
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

#[cfg(test)]
mod tests;
