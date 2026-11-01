//! Retained output and suspended block admission.
mod publish;
mod run;
use super::Lexer;
use crate::{
    Kind, MappedText, MarkedOptions, Node,
    inline::ScopeKey,
    rules,
    source::{ScopeSource, Segmented, Source},
    tokenizer,
    tokenizer::block::ListState,
};
pub(crate) use run::{BlockRun, Level};
use std::{collections::HashMap, ops::Range};

/// Paragraph admission state carried across one level's dispatch steps.
#[derive(Default)]
pub(crate) struct ParagraphState {
    /// Clipping carried from the preceding paragraph step.
    clipped: bool,
    /// Setext lookahead for item text admitted one line at a time.
    item: Option<rules::paragraph::ItemText>,
}

/// Effective grammar tail, including an invisible retained definition.
#[derive(Clone)]
enum Tail {
    /// Last visible block's own raw source when it can reenter a container.
    Visible(Option<Segmented>),
    /// A browser definition prevents merges with the preceding visible block.
    Definition,
}

/// One ordered output carried across a quote's chunk levels.
#[derive(Default)]
pub(crate) struct BlockTokens {
    /// Visible semantic nodes.
    pub(crate) nodes: Vec<Node>,
    /// Inline operands retained as shared source until this invocation publishes.
    pub(crate) scopes: HashMap<ScopeKey, Segmented>,
    /// Last grammar token, independent of visible output.
    tail: Tail,
    /// Admission history of the visible trailing list, if present.
    list: Option<Box<ListState>>,
}
impl Default for Tail {
    fn default() -> Self {
        Self::Visible(None)
    }
}

/// Output before a dispatch step whose admission can still change.
#[derive(Clone, Default)]
pub(crate) struct Mark {
    /// Visible nodes before the step.
    nodes: usize,
    /// Spelling, extent and inline operand of the preceding node, which the step may extend.
    last: Option<(MappedText, Range<usize>, Option<Segmented>)>,
    /// Grammar tail before the step.
    tail: Tail,
    /// Paragraph-versus-item grammar position before the step.
    pub(super) top: bool,
}

impl Mark {
    /// Capture the handles a following step can change, without copying their text.
    pub(super) fn new(output: &BlockTokens, top: bool) -> Self {
        Self {
            nodes: output.nodes.len(),
            last: output.nodes.last().map(|node| {
                let key = (node.range.start, node.range.end, false);
                let scope = output.scopes.get(&key).cloned();
                (node.authored.clone(), node.range.clone(), scope)
            }),
            tail: output.tail.clone(),
            top,
        }
    }
}

impl BlockTokens {
    /// Current visible tail only when no invisible definition intervenes.
    pub(super) fn last(&self) -> Option<&Node> {
        match self.tail {
            Tail::Visible(_) => self.nodes.last(),
            Tail::Definition => None,
        }
    }

    /// Publish a block and retain a container's own recognized raw source.
    pub(super) fn push(&mut self, source: &Source<'_>, consumed: usize, node: Node) {
        self.complete();
        self.tail = Tail::Visible(
            matches!(node.kind, Kind::List(..) | Kind::Quote(..))
                .then(|| source.select(std::iter::once(0..consumed)).into()),
        );
        self.nodes.push(node);
    }

    /// Publish a block recognized over retained segmented input.
    pub(super) fn push_segmented(&mut self, raw: Segmented, node: Node) {
        self.complete();
        self.tail =
            Tail::Visible(matches!(node.kind, Kind::List(..) | Kind::Quote(..)).then_some(raw));
        self.nodes.push(node);
    }

    /// Publish a container's own node together with the item history it carries, so a
    /// published list is never separated from the slots its history fills.
    pub(super) fn push_container(
        &mut self,
        raw: Segmented,
        node: Node,
        list: Option<Box<ListState>>,
    ) {
        self.push_segmented(raw, node);
        self.list = list;
    }

    /// Take the trailing list's item history for publication.
    pub(super) fn take_list(&mut self) -> Option<Box<ListState>> {
        self.list.take()
    }

    /// Drop contributions admitted after `mark`, returning the removed nodes, and restore the
    /// preceding node that a dropped step may have extended.
    pub(super) fn restore(&mut self, mark: Mark) -> Vec<Node> {
        let removed: Vec<_> = self.nodes.drain(mark.nodes..).collect();
        if !removed.is_empty() {
            self.list = None;
        }
        for node in &removed {
            super::frames::forget(&mut self.scopes, node);
        }
        if let (Some(node), Some((authored, range, scope))) = (self.nodes.last_mut(), mark.last) {
            self.scopes
                .remove(&(node.range.start, node.range.end, false));
            if let Some(scope) = scope {
                self.scopes.insert((range.start, range.end, false), scope);
            }
            node.authored = authored;
            node.range = range;
        }
        self.tail = mark.tail;
        removed
    }

    /// Merge ordinary code into eligible text using the one dedented operand.
    pub(super) fn code(&mut self, source: &Segmented) -> Option<usize> {
        let block = tokenizer::block::code(source)?;
        let consumed = block.consumed;
        if !self
            .last()
            .is_some_and(|node| matches!(node.kind, Kind::Paragraph(_) | Kind::Inline(_)))
        {
            self.push_segmented(source.select(0..consumed), block.node);
            return Some(consumed);
        }
        let end = code_merge_end(source, consumed);
        let raw = source.select(0..end);
        let range = raw.native(&(0..end))?;
        let body = block.inline?;
        let end_text = body.local(&range)?.end;
        let end_text = body.text[..end_text].trim_end_matches('\n').len();
        let body = body.select(std::iter::once(0..end_text));
        let node = Node {
            authored: raw.mapped(),
            range,
            kind: Kind::Inline(Vec::new()),
        };
        self.merge(&node, &body.into());
        Some(end)
    }

    /// Extend the semantic tail and transfer its inline scope to the new extent.
    fn merge(&mut self, node: &Node, body: &Segmented) {
        let Some(last) = self.nodes.last_mut() else {
            return;
        };
        let key = (last.range.start, last.range.end, false);
        let Some(mut previous) = self.scopes.remove(&key) else {
            return;
        };
        if !previous.mapped().ends_with_newline() {
            previous.newline();
        }
        previous.append(body);
        if !last.authored.ends_with_newline() {
            last.authored.append(&"\n".to_owned().into());
        }
        last.authored.append(&node.authored);
        last.range.end = node.range.end;
        self.scopes
            .insert((last.range.start, last.range.end, false), previous);
        self.tail = Tail::Visible(None);
    }

    /// Borrow the trailing container's raw source for reentry.
    pub(super) fn raw(&mut self) -> Option<Segmented> {
        match &mut self.tail {
            Tail::Visible(raw) => raw.take(),
            Tail::Definition => None,
        }
    }
}
impl<E> Lexer<'_, E> {
    /// Publish completed block output and materialize each inline operand once.
    pub(super) fn finish_blocks(&mut self, mut output: BlockTokens) -> Vec<Node> {
        output.complete();
        self.scopes.extend(
            output
                .scopes
                .into_iter()
                .map(|(key, source)| (key, source.materialize())),
        );
        output.nodes
    }

    /// Admit a paragraph that may merge into the visible tail.
    fn paragraph_step(
        &mut self,
        source: &Segmented,
        output: &mut BlockTokens,
        caller: Option<&Source<'_>>,
        state: &mut ParagraphState,
    ) -> Option<usize> {
        self.clipped = false;
        let (consumed, node) = self.paragraph(source, output, caller, state.item.as_mut())?;
        let inline = matches!(node.kind, Kind::Inline(_));
        let merge = output.last().is_some_and(|last| {
            inline && matches!(last.kind, Kind::Inline(_))
                || state.clipped
                    && matches!(node.kind, Kind::Paragraph(_))
                    && matches!(last.kind, Kind::Paragraph(_))
        });
        if merge {
            Self::merge_node(output, &node);
        } else {
            output.push_segmented(source.select(0..consumed), node);
        }
        state.clipped = self.clipped;
        Some(consumed)
    }

    /// Admit every block kind that decides before container recognition.
    fn block_lead(
        &mut self,
        source: &Segmented,
        output: &mut BlockTokens,
        callback: Option<&Source<'_>>,
    ) -> Option<usize> {
        if let Some(consumed) = callback.and_then(|source| self.extension(source, output)) {
            return Some(consumed);
        }
        let first = source.first_line();
        let first = first.view(0..first.text.len());
        if let Some(consumed) = Self::space_block(&first, output).or_else(|| output.code(source)) {
            return Some(consumed);
        }
        let block = tokenizer::block::leaf(source, &first)?;
        if let Some(inline) = block.inline {
            output.scopes.insert(
                (block.node.range.start, block.node.range.end, false),
                inline.into(),
            );
        }
        output.push_segmented(source.select(0..block.consumed), block.node);
        Some(block.consumed)
    }

    /// Admit the block kinds that decide after container recognition.
    fn block_tail(&mut self, source: &Segmented, output: &mut BlockTokens) -> Option<usize> {
        if let Some(block) = tokenizer::block::html(source, self.options) {
            output.push_segmented(source.select(0..block.consumed), block.node);
            return Some(block.consumed);
        }
        self.definition_block(source, output).or_else(|| {
            self.table(source, output).map(|(consumed, node)| {
                output.push_segmented(source.select(0..consumed), node);
                consumed
            })
        })
    }

    /// Consume spacing while keeping the visible tail's raw and scope in sync.
    fn space_block(source: &Source<'_>, output: &mut BlockTokens) -> Option<usize> {
        #[cfg(test)]
        crate::tests::block_operand(source.text);
        let gap = rules::BLOCK_NEWLINE
            .as_ref()?
            .find(source.text)
            .filter(|gap| gap.end() > 0)?;
        if gap.end() != 1 || output.last().is_none() {
            output.push(source, gap.end(), source.node(gap.range(), Kind::Gap));
            return Some(gap.end());
        }
        if let Some(last) = output.nodes.last_mut() {
            let scope = output
                .scopes
                .remove(&(last.range.start, last.range.end, false));
            last.authored.append(&source.authored(0..1));
            last.range.end = source.extent(0..1).end;
            if let Some(scope) = scope {
                output
                    .scopes
                    .insert((last.range.start, last.range.end, false), scope);
            }
        }
        if let Tail::Visible(Some(raw)) = &mut output.tail {
            raw.append(&source.select(std::iter::once(0..1)).into());
        }
        Some(gap.end())
    }

    /// Merge literal definitions before registering otherwise first-wins references.
    fn definition_block(&mut self, source: &Segmented, output: &mut BlockTokens) -> Option<usize> {
        let (matched, undecided) = rules::definition::recognize(source, self.options);
        self.undecided |= undecided;
        let matched = matched?;
        let raw = source.select(0..matched.consumed).materialize();
        let source = &raw.view(0..raw.text.len());
        if output
            .last()
            .is_some_and(|node| matches!(node.kind, Kind::Paragraph(_) | Kind::Inline(_)))
        {
            Self::merge_source(
                output,
                source,
                matched.consumed,
                source.select(std::iter::once(0..matched.consumed)),
            );
        } else if self.definition(source, &matched)
            && matches!(self.options, MarkedOptions::Browser { .. })
        {
            output.complete();
            output.tail = Tail::Definition;
        }
        Some(matched.consumed)
    }

    /// Join inline operands without reconstructing them from bounding node ranges.
    fn merge_node(output: &mut BlockTokens, node: &Node) {
        if let Some(body) = output
            .scopes
            .remove(&(node.range.start, node.range.end, false))
        {
            output.merge(node, &body);
        }
    }

    /// Join a dedented or literal operand while preserving its original provenance.
    fn merge_source(
        output: &mut BlockTokens,
        source: &Source<'_>,
        consumed: usize,
        body: ScopeSource,
    ) {
        let node = source.node(0..consumed, Kind::Inline(Vec::new()));
        output.merge(&node, &body.into());
    }
}

/// Preserve authored blank lines after merged code, not synthetic reentry seams.
fn code_merge_end(source: &Segmented, consumed: usize) -> usize {
    use crate::rules::lines::Input;
    let raw = source.select(0..consumed);
    let mut lines = rules::lines::Cursor::default();
    let mut end = consumed;
    let mut blank = false;
    while let Some(range) = lines.next(&raw).or_else(|| lines.finish(&raw)) {
        let line = raw.window(range.clone());
        #[cfg(test)]
        crate::tests::block_operand(&line);
        if line.trim_matches([' ', '\t', '\n']).is_empty() {
            blank |= raw.native(&range).is_some_and(|range| !range.is_empty());
        } else {
            end = range.end - usize::from(line.ends_with('\n'));
            blank = false;
        }
    }
    if blank { end } else { consumed }
}

#[cfg(test)]
pub(super) mod tests;
