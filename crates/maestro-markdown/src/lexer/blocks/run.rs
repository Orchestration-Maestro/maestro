//! Suspended block admission over one operand.
use super::{BlockTokens, Lexer, Mark};
use crate::{
    lexer::{
        Frame,
        frames::{Done, Pending, Step},
    },
    source::{ScopeSource, Segmented},
};

/// Which caller owns one block admission level.
#[derive(Clone, Copy)]
pub(crate) enum Level {
    /// The document's own level.
    Document,
    /// A quote chunk, which starts with clipping and restores the enclosing grammar.
    Chunk,
    /// A list item body, which retains its first open step for re-admission.
    Item,
}

/// First open step of one block admission level.
enum Open {
    /// This level does not retain an open step.
    Untracked,
    /// No step has yet read the operand's apparent end.
    Unread,
    /// Offset and output mark of the first step that did.
    At(usize, Box<Mark>),
}

/// One block admission level, suspended while a container admits its body.
pub(crate) struct BlockRun {
    /// Operand admitted at this level.
    source: Segmented,
    /// Next logical byte to admit.
    cursor: usize,
    /// Paragraph clipping carried across this level's steps.
    clipped: bool,
    /// Caller tokenizer view over this level's operand.
    callback: Option<ScopeSource>,
    /// First open step, retained while an enclosing item can re-admit it.
    open: Open,
    /// Enclosing undecided flag restored when this level completes.
    undecided: bool,
    /// Retained output of this level.
    output: BlockTokens,
    /// Dispatch step awaiting a container level.
    awaiting: Option<Await>,
    /// Whether no further block can be admitted here.
    stopped: bool,
}

/// One dispatch step that cannot finish before its container completes.
struct Await {
    /// Cursor offset the step started at.
    cursor: usize,
    /// Open-step mark captured before the step.
    mark: Option<Box<Mark>>,
    /// Container grammar still available to this step.
    fallback: Fallback,
}

/// Container grammar a declined step falls back to.
#[derive(Clone, Copy)]
enum Fallback {
    /// The list grammar, after a quote prefix declined.
    List,
    /// The block kinds that decide after container recognition.
    Tail,
}

impl BlockRun {
    /// Start one admission level over an operand and its retained output.
    fn new(
        source: Segmented,
        output: BlockTokens,
        level: Level,
        callback: Option<ScopeSource>,
        undecided: bool,
    ) -> Self {
        Self {
            source,
            cursor: 0,
            clipped: matches!(level, Level::Chunk),
            callback,
            open: match level {
                Level::Item => Open::Unread,
                Level::Document | Level::Chunk => Open::Untracked,
            },
            undecided,
            output,
            awaiting: None,
            stopped: false,
        }
    }

    /// Admit blocks until this level completes or a container takes over.
    pub(crate) fn step<E>(&mut self, lexer: &mut Lexer<'_, E>) -> Step {
        while self.awaiting.is_none()
            && !self.stopped
            && self.cursor < self.source.len()
            && lexer.error.is_none()
        {
            let start = self.cursor;
            let rest = self.source.select(start..self.source.len());
            let mark = matches!(self.open, Open::Unread)
                .then(|| Box::new(Mark::new(&self.output, lexer.top)));
            lexer.undecided = false;
            let caller = self
                .callback
                .as_ref()
                .map(|source| source.view(start..source.text.len()));
            if let Some(consumed) = lexer.block_lead(&rest, &mut self.output, caller.as_ref()) {
                self.admitted(lexer, &rest, (start, mark), consumed);
                continue;
            }
            if let Some(pending) = lexer.container(&rest) {
                let fallback = match pending {
                    Pending::Quote(_) => Fallback::List,
                    Pending::List(_) => Fallback::Tail,
                };
                self.awaiting = Some(Await {
                    cursor: start,
                    mark,
                    fallback,
                });
                return Step::Push(lexer.admit(pending));
            }
            if !self.tail(lexer, &rest, (start, mark)) {
                self.stopped = true;
            }
        }
        if self.awaiting.is_some() {
            return Step::Again;
        }
        lexer.undecided = self.undecided;
        lexer.top = true;
        self.stopped = true;
        let open = match std::mem::replace(&mut self.open, Open::Untracked) {
            Open::At(cursor, mark) => Some((cursor, *mark)),
            Open::Unread | Open::Untracked => None,
        };
        Step::Done(Done::Blocks(std::mem::take(&mut self.output), open))
    }

    /// Continue after the container level this run suspended for completed.
    pub(crate) fn resumed<E>(&mut self, lexer: &mut Lexer<'_, E>, done: Done) -> Option<Frame> {
        let awaiting = self.awaiting.take()?;
        match done {
            Done::Consumed {
                consumed,
                node,
                list,
            } => {
                let start = awaiting.cursor;
                self.output
                    .push_segmented(self.source.select(start..start + consumed), node);
                self.output.list = list;
                let rest = self.source.select(start..self.source.len());
                self.admitted(lexer, &rest, (start, awaiting.mark), consumed);
                None
            }
            Done::Rejected => self.declined(lexer, awaiting),
            Done::Blocks(..) | Done::Resumed { .. } => None,
        }
    }

    /// Continue one dispatch step after a container grammar declined it.
    fn declined<E>(&mut self, lexer: &mut Lexer<'_, E>, awaiting: Await) -> Option<Frame> {
        let Await {
            cursor,
            mark,
            fallback,
        } = awaiting;
        let rest = self.source.select(cursor..self.source.len());
        if matches!(fallback, Fallback::List)
            && let Some(entry) = lexer.list_entry(&rest)
        {
            self.awaiting = Some(Await {
                cursor,
                mark,
                fallback: Fallback::Tail,
            });
            return Some(lexer.admit(Pending::List(entry)));
        }
        if !self.tail(lexer, &rest, (cursor, mark)) {
            self.stopped = true;
        }
        None
    }

    /// Admit the tail grammars at one position, reporting whether one consumed bytes.
    fn tail<E>(
        &mut self,
        lexer: &mut Lexer<'_, E>,
        rest: &Segmented,
        step: (usize, Option<Box<Mark>>),
    ) -> bool {
        let caller = self
            .callback
            .as_ref()
            .map(|source| source.view(step.0..source.text.len()));
        let consumed = lexer.block_tail(rest, &mut self.output).or_else(|| {
            lexer.paragraph_step(rest, &mut self.output, caller.as_ref(), &mut self.clipped)
        });
        if let Some(consumed) = consumed {
            self.admitted(lexer, rest, step, consumed);
            true
        } else {
            self.record(lexer, rest, step, None);
            false
        }
    }

    /// Keep this level's position after one dispatch step admitted `consumed` bytes.
    fn admitted<E>(
        &mut self,
        lexer: &Lexer<'_, E>,
        rest: &Segmented,
        step: (usize, Option<Box<Mark>>),
        consumed: usize,
    ) {
        let cursor = step.0;
        self.record(lexer, rest, step, Some(consumed));
        self.cursor = cursor + consumed;
    }

    /// Keep the first open step: its grammar or a caller tokenizer read the operand's end, or
    /// the content after it sits on the operand's unterminated last line.
    fn record<E>(
        &mut self,
        lexer: &Lexer<'_, E>,
        source: &Segmented,
        step: (usize, Option<Box<Mark>>),
        consumed: Option<usize>,
    ) {
        let (cursor, Some(mark)) = step else {
            return;
        };
        if matches!(self.open, Open::Unread)
            && (lexer.tokenizer.is_some()
                || lexer.undecided
                || consumed.is_none_or(|consumed| !source.settled_after(consumed)))
        {
            self.open = Open::At(cursor, mark);
        }
    }
}

impl<E> Lexer<'_, E> {
    /// Suspend this level under a block admission of one operand into its output.
    pub(crate) fn block_run(
        &mut self,
        source: Segmented,
        output: BlockTokens,
        level: Level,
    ) -> BlockRun {
        let callback = self.tokenizer.as_ref().map(|_| source.materialize());
        let undecided = std::mem::take(&mut self.undecided);
        BlockRun::new(source, output, level, callback, undecided)
    }
}
