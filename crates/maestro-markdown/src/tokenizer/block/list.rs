//! List admission and item continuation framing.
use crate::{
    Kind, MarkedOptions, Node,
    lexer::{BlockTokens, Child, Done, Frame, Level, Lexer, Step},
    rules,
    source::{Cursor, ScopeSource, Segmented},
    tokenizer::capture,
};
use maestro_text::is_whitespace_scalar;
mod state;
use state::{CompletedItem, ItemLines};

/// Marker identity and indentation at the start of one list item.
struct Marker {
    /// Ordered value, absent for bullets.
    start: Option<u64>,
    /// Delimiter or unordered bullet.
    bullet: u8,
    /// Complete first-line extent.
    line: usize,
    /// End of the marker before first-line whitespace.
    prefix: usize,
    /// Continuation indentation.
    indent: usize,
    /// Leading first-line tabs expanded before measuring content.
    tabs: usize,
}

impl Marker {
    /// Keep orderedness and the selected bullet or delimiter within one list.
    fn follows(&self, first: &Self) -> bool {
        self.start.is_some() == first.start.is_some() && self.bullet == first.bullet
    }
}

/// Recognize a marker without treating a horizontal rule as an item.
fn marker(source: &str) -> Option<Marker> {
    #[cfg(test)]
    crate::tests::block_operand(source);
    let matched = rules::LIST_ITEM.as_ref()?.find(source)?;
    #[cfg(test)]
    crate::tests::block_operand(source);
    if rules::BLOCK_HR.as_ref()?.find(source).is_some() {
        return None;
    }
    let raw = capture(source, &matched, 1);
    let bullet = *raw.as_bytes().last()?;
    let ordered = raw.trim_start().len() > 1;
    let start = ordered
        .then(|| raw.trim().trim_end_matches(['.', ')']).parse().ok())
        .flatten();
    let line = matched.end();
    let rest = &source[raw.len()..line];
    let blank = rest.trim_matches(is_whitespace_scalar).is_empty();
    let spaces = rest.bytes().take_while(|byte| *byte == b' ').count();
    let indent = if blank || spaces > 4 { 1 } else { spaces };
    let tabs = if blank {
        0
    } else {
        rest.bytes().take_while(|byte| *byte == b'\t').count()
    };
    Some(Marker {
        start,
        bullet,
        line,
        prefix: raw.len(),
        indent: raw.len() + indent,
        tabs,
    })
}

/// Retained list identity and trailing item's admission history.
pub(crate) struct ListState {
    /// The first marker fixes orderedness and delimiter identity.
    first: Marker,
    /// Framing and source of the final item, retained until the next boundary.
    item: ItemLines,
    /// Logical start of the final item within its list operand.
    item_start: usize,
    /// Stopped items as publication and a caller replay read them, in authored order.
    completed: Vec<CompletedItem>,
}

impl ListState {
    /// Complete the trailing item and admit the next marker's first line.
    fn begin(&mut self, line: &ScopeSource, next: &Marker, options: MarkedOptions) {
        let previous = std::mem::replace(&mut self.item, ItemLines::new(line, next, options));
        self.completed.push(previous.complete());
    }

    /// The item this level admits, from a replayed position or the trailing item.
    fn item_mut(&mut self, replay: Option<usize>) -> Option<ItemSlot<'_>> {
        match replay {
            Some(index) => self.completed.get_mut(index).map(ItemSlot::Stopped),
            None => Some(ItemSlot::Open(&mut self.item)),
        }
    }

    /// Produce each item's output and whether the list is loose.
    pub(crate) fn outputs(mut self) -> (Vec<BlockTokens>, bool) {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| {
            let mut retained = crate::tests::Retained::default();
            for item in &self.completed {
                item.retained(&mut retained);
            }
            self.item.retained(&mut retained);
            work.frame_bytes = work.frame_bytes.max(retained.bytes);
        });
        let following = self.completed.len();
        let mut outputs = Vec::with_capacity(following + 1);
        let mut loose = false;
        for item in &mut self.completed {
            let output = item.finish();
            loose |= item.blank_end() || blank_separator(&output);
            outputs.push(output);
        }
        let output = self.item.child.finish(task(self.item.body()).1);
        loose |= blank_separator(&output);
        outputs.push(output);
        (outputs, loose)
    }
}

/// The item one list level admits: the open trailing item or a stopped one replayed.
enum ItemSlot<'a> {
    /// The trailing item, still admitting its own continuation lines.
    Open(&'a mut ItemLines),
    /// A stopped item a caller tokenizer re-admits.
    Stopped(&'a mut CompletedItem),
}

impl ItemSlot<'_> {
    /// Borrow the dedented semantic body this item's child admits.
    fn body(&self) -> &Segmented {
        match self {
            Self::Open(item) => item.body(),
            Self::Stopped(item) => item.body(),
        }
    }

    /// The final semantic boundary only the trailing item's trim still reads.
    fn body_end(&self) -> Option<usize> {
        match self {
            Self::Open(item) => Some(item.body_end()),
            Self::Stopped(_) => None,
        }
    }

    /// Borrow the child invocation the item's body admits into.
    fn child(&mut self) -> &mut Child {
        match self {
            Self::Open(item) => &mut item.child,
            Self::Stopped(item) => item.child(),
        }
    }
}

/// Whether an item's output opens with a blank separator of its own, which loosens the list.
fn blank_separator(output: &BlockTokens) -> bool {
    output.nodes.iter().any(|node| {
        matches!(node.kind, Kind::Gap)
            && node
                .authored
                .chunks()
                .flat_map(str::bytes)
                .filter(|byte| *byte == b'\n')
                .count()
                > 1
    })
}

/// A recognized list whose items are admitted by their own levels.
pub(crate) struct ListEntry {
    /// Complete list operand.
    pub(crate) source: Segmented,
    /// Retained list identity and trailing item.
    pub(crate) state: Box<ListState>,
    /// Bytes the first marker already consumed.
    pub(crate) offset: usize,
}

/// Where a completed list level delivers its node.
pub(crate) enum ListTarget {
    /// A container step in an enclosing block level.
    Container,
    /// The enclosing quote level, which publishes this node into its output.
    Resumed,
}

/// One list level, suspended while an item admits its body.
pub(crate) struct ListFrame {
    /// Complete list operand.
    source: Segmented,
    /// Retained list identity, trailing item and completed items.
    state: Option<Box<ListState>>,
    /// Logical offset of the next unadmitted line.
    offset: usize,
    /// Published item wrappers, in authored order.
    items: Vec<Node>,
    /// Remaining physical lines.
    pending: Cursor,
    /// Where this level delivers its node.
    target: ListTarget,
    /// Next action of this level.
    phase: ListPhase,
}

/// Next action of one list level.
enum ListPhase {
    /// Replay completed items for a caller tokenizer, from this index.
    Replay(usize),
    /// Scan the remaining physical lines.
    Lines,
    /// Await one item's admission; `next` begins the following item.
    Item {
        /// Following item's first line, when a marker begins one.
        next: Option<Next>,
        /// Completed item being replayed for a caller tokenizer.
        replay: Option<usize>,
        /// Operand of the item whose body a child level admits.
        awaiting: Option<Segmented>,
    },
    /// The list cannot be published.
    Reject,
    /// Publish the completed list.
    Publish,
}

/// A following item's first line and its marker.
struct Next {
    /// Complete first physical line of the following item.
    line: ScopeSource,
    /// Marker identity of the following item.
    marker: Marker,
}

impl ListFrame {
    /// Start one list level over its recognized operand.
    pub(crate) fn new(entry: ListEntry, target: ListTarget) -> Self {
        Self::start(entry.source, entry.state, entry.offset, Vec::new(), target)
    }

    /// Start one list level resuming a trailing item's retained history.
    pub(crate) fn resuming(
        source: Segmented,
        mut state: Box<ListState>,
        items: Vec<Node>,
        target: ListTarget,
    ) -> Self {
        let start = state.item_start;
        let offset = state.item.resume(&source, start);
        Self::start(source, state, offset, items, target)
    }

    /// Keep the wrappers of the completed items and the list's identity.
    fn start(
        source: Segmented,
        state: Box<ListState>,
        offset: usize,
        mut items: Vec<Node>,
        target: ListTarget,
    ) -> Self {
        items.truncate(state.completed.len());
        Self {
            source,
            state: Some(state),
            offset,
            items,
            pending: Cursor::default(),
            target,
            phase: ListPhase::Replay(0),
        }
    }

    /// Scan lines, admit the trailing item and publish this level.
    pub(crate) fn step<E>(&mut self, lexer: &mut Lexer<'_, E>) -> Step {
        loop {
            let step = match self.phase {
                ListPhase::Replay(index) => self.replay(lexer, index),
                ListPhase::Lines => self.lines(lexer),
                ListPhase::Item { .. } => return Step::Again,
                ListPhase::Reject => return Step::Done(Done::Rejected),
                ListPhase::Publish => return self.publish(lexer),
            };
            if let Some(step) = step {
                return step;
            }
        }
    }

    /// Replay one completed item for a caller tokenizer, or move on to the lines.
    fn replay<E>(&mut self, lexer: &mut Lexer<'_, E>, index: usize) -> Option<Step> {
        if lexer.tokenizer.is_some() && index < self.completed() {
            return Some(self.begin_item(lexer, true, None, Some(index)));
        }
        self.phase = ListPhase::Lines;
        self.pending
            .segmented(self.source.select(self.offset..self.source.len()));
        None
    }

    /// Admit the next physical line, or admit the trailing item once lines end.
    fn lines<E>(&mut self, lexer: &mut Lexer<'_, E>) -> Option<Step> {
        let Some(line) = self.pending.line() else {
            return Some(self.begin_item(lexer, false, None, None));
        };
        let line = line.materialize();
        let state = self.state.as_deref_mut()?;
        if state.item.append(&line) {
            self.offset += line.text.len();
            return None;
        }
        let next = marker(&line.text).filter(|next| next.follows(&state.first));
        Some(match next {
            Some(marker) => self.begin_item(lexer, true, Some(Next { line, marker }), None),
            None => self.begin_item(lexer, false, None, None),
        })
    }

    /// Continue after the block level admitting one item's body completed.
    pub(crate) fn resumed<E>(&mut self, lexer: &mut Lexer<'_, E>, done: Done) -> Option<Frame> {
        let Done::Blocks(output, open) = done else {
            return None;
        };
        let ListPhase::Item {
            replay, awaiting, ..
        } = &mut self.phase
        else {
            return None;
        };
        let body = awaiting.take()?;
        let replay = *replay;
        let state = self.state.as_deref_mut()?;
        if let Some(mut item) = state.item_mut(replay) {
            let child = item.child();
            child.restore_output(output);
            lexer.settle(&body, child, open);
        }
        self.item_finished(lexer);
        None
    }

    /// Begin admitting one item's body, or continue when it has nothing to admit.
    fn begin_item<E>(
        &mut self,
        lexer: &mut Lexer<'_, E>,
        following: bool,
        next: Option<Next>,
        replay: Option<usize>,
    ) -> Step {
        let Some(state) = self.state.as_deref_mut() else {
            self.phase = ListPhase::Reject;
            return Step::Done(Done::Rejected);
        };
        let Some(mut item) = state.item_mut(replay) else {
            self.phase = ListPhase::Reject;
            return Step::Done(Done::Rejected);
        };
        // Only the final list item is trimmed of its trailing whitespace.
        let end = item.body_end().filter(|_| !following);
        let (body, prefix) = operand(item.body(), end);
        let child = item.child();
        let rest = lexer.reopen(&body, prefix, child);
        self.phase = ListPhase::Item {
            next,
            replay,
            awaiting: rest.is_some().then_some(body),
        };
        let Some(rest) = rest else {
            lexer.top = true;
            self.item_finished(lexer);
            return Step::Again;
        };
        lexer.top = child.enter_top();
        let output = child.take_output();
        Step::Push(Frame::Run(lexer.block_run(rest, output, Level::Item)))
    }

    /// Run the actions that follow one item's admission.
    fn item_finished<E>(&mut self, lexer: &Lexer<'_, E>) {
        let ListPhase::Item { next, replay, .. } =
            std::mem::replace(&mut self.phase, ListPhase::Reject)
        else {
            return;
        };
        if let Some(index) = replay {
            self.phase = ListPhase::Replay(index + 1);
            return;
        }
        self.phase = ListPhase::Lines;
        let Some(state) = self.state.as_deref_mut() else {
            return;
        };
        let Some(node) = wrapper(&state.item) else {
            self.phase = ListPhase::Reject;
            return;
        };
        self.items.push(node);
        let Some(Next { line, marker }) = next else {
            self.phase = ListPhase::Publish;
            return;
        };
        state.item_start = self.offset;
        state.begin(&line, &marker, lexer.options);
        self.offset += marker.line;
    }

    /// Number of completed items a caller tokenizer replays.
    fn completed(&self) -> usize {
        self.state
            .as_deref()
            .map_or(0, |state| state.completed.len())
    }

    /// Publish the completed list into the level that suspended for it.
    fn publish<E>(&mut self, lexer: &mut Lexer<'_, E>) -> Step {
        lexer.top = true;
        let Some(state) = self.state.take() else {
            return Step::Done(Done::Rejected);
        };
        let end = state.item_start + state.item.raw_end();
        let raw = self.source.select(0..end);
        let Some(range) = raw.native(&(0..end)) else {
            return Step::Done(Done::Rejected);
        };
        let node = Node {
            range,
            authored: raw.mapped(),
            kind: Kind::List(state.first.start, std::mem::take(&mut self.items)),
        };
        match self.target {
            ListTarget::Container => Step::Done(Done::Consumed {
                consumed: end,
                node,
                list: Some(state),
            }),
            ListTarget::Resumed => Step::Done(Done::Resumed {
                used: raw,
                node,
                rest: self.source.select(end..self.source.len()),
                list: Some(state),
            }),
        }
    }
}

impl<E> Lexer<'_, E> {
    /// Recognize a list marker without admitting its items.
    pub(crate) fn list_entry(&mut self, source: &Segmented) -> Option<ListEntry> {
        let first = source.first_line();
        let first_line = first.text.split_inclusive('\n').next()?;
        #[cfg(test)]
        crate::tests::block_operand(first_line);
        rules::BLOCK_LIST.as_ref()?.find(first_line)?;
        let first = marker(first_line)?;
        let item = ItemLines::new(
            &source.select(0..first.line).materialize(),
            &first,
            self.options,
        );
        let offset = first.line;
        Some(ListEntry {
            source: source.clone(),
            state: Box::new(ListState {
                first,
                item,
                item_start: 0,
                completed: Vec::new(),
            }),
            offset,
        })
    }
}

/// Bytes one finished list item retains until its list publishes.
#[cfg(test)]
pub(crate) fn completed_record_bytes() -> usize {
    std::mem::size_of::<state::CompletedItem>()
}

/// Select one item's child operand, trimmed only for the final list item.
fn operand(source: &Segmented, end: Option<usize>) -> (Segmented, usize) {
    let (mut body, _) = task(source);
    let prefix = source.len() - body.len();
    if let Some(end) = end {
        body.truncate(end.saturating_sub(prefix));
    }
    (body, prefix)
}

/// Publish an item's provisional wrapper over its recognized raw.
fn wrapper(item: &ItemLines) -> Option<Node> {
    let raw = item.raw();
    Some(Node {
        authored: raw.mapped(),
        range: raw.native(&(0..raw.len()))?,
        kind: Kind::Item(Vec::new()),
    })
}

/// Task markers are consumed before the item's ordinary inline grammar.
fn task(source: &Segmented) -> (Segmented, Option<bool>) {
    let first = source.first_line();
    let bytes = first.text.as_bytes();
    if bytes.len() >= 4
        && bytes[0] == b'['
        && bytes[2] == b']'
        && bytes[3] == b' '
        && matches!(bytes[1], b' ' | b'x' | b'X')
    {
        let end = 3 + first.text[3..]
            .bytes()
            .take_while(|byte| *byte == b' ')
            .count();
        let checked = bytes[1] != b' ';
        (source.select(end..source.len()), Some(checked))
    } else {
        (source.clone(), None)
    }
}

/// Loose items promote line text without changing genuine paragraphs.
pub(crate) fn loosen(item: &mut Node) {
    let Kind::Item(children) = &mut item.kind else {
        return;
    };
    for child in children {
        if let Kind::Inline(content) = &mut child.kind {
            child.kind = Kind::Paragraph(std::mem::take(content));
        }
    }
}
