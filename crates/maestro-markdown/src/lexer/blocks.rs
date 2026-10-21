//! Retained output and ordered recursive block admission.
use super::Lexer;
use crate::{
    Kind, MarkedOptions, Node,
    inline::ScopeKey,
    rules,
    source::{ScopeSource, Segmented, Source},
    tokenizer,
};
use std::collections::HashMap;

/// Effective grammar tail, including an invisible retained definition.
enum Tail {
    /// Last visible block's own raw source when it can reenter a container.
    Visible(Option<Segmented>),
    /// A browser definition prevents merges with the preceding visible block.
    Definition,
}

/// One ordered output shared across recursive quote chunks.
#[derive(Default)]
pub(crate) struct BlockTokens {
    /// Visible semantic nodes.
    pub(crate) nodes: Vec<Node>,
    /// Inline operands retained as shared source until this invocation publishes.
    pub(crate) scopes: HashMap<ScopeKey, Segmented>,
    /// Last grammar token, independent of visible output.
    tail: Tail,
    /// Admission history of the visible trailing list, if present.
    pub(super) list: Option<Box<tokenizer::block::ListState>>,
}
impl Default for Tail {
    fn default() -> Self {
        Self::Visible(None)
    }
}

impl BlockTokens {
    /// Publish the trailing list's children only when its invocation completes.
    pub(crate) fn complete(&mut self) {
        if let Some(list) = self.list.take()
            && let Some(Node {
                kind: Kind::List(_, items),
                ..
            }) = self.nodes.last_mut()
        {
            list.finish(items, &mut self.scopes);
        }
    }

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

    /// Append blocks while preserving the invocation's paragraph clipping state.
    pub(super) fn block_tokens(
        &mut self,
        source: &Segmented,
        output: &mut BlockTokens,
        mut clipped: bool,
    ) {
        let callback = self.tokenizer.as_ref().map(|_| source.materialize());
        let mut cursor = 0;
        while cursor < source.len() && self.error.is_none() {
            let rest = source.select(cursor..source.len());
            let caller = callback
                .as_ref()
                .map(|source| source.view(cursor..source.text.len()));
            if let Some(consumed) = self.block(&rest, output, caller.as_ref()) {
                cursor += consumed;
                continue;
            }
            self.clipped = false;
            let Some((consumed, node)) = self.paragraph(&rest, output, caller.as_ref()) else {
                break;
            };
            let inline = matches!(node.kind, Kind::Inline(_));
            let merge = output.last().is_some_and(|last| {
                inline && matches!(last.kind, Kind::Inline(_))
                    || clipped
                        && matches!(node.kind, Kind::Paragraph(_))
                        && matches!(last.kind, Kind::Paragraph(_))
            });
            if merge {
                Self::merge_node(output, &node);
            } else {
                output.push_segmented(rest.select(0..consumed), node);
            }
            clipped = self.clipped;
            cursor += consumed;
        }
        self.top = true;
    }

    /// Admit everything preceding paragraph and line-text fallback.
    fn block(
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
        if let Some(consumed) =
            Self::space_block(&first, output).or_else(|| Self::code_block(source, output))
        {
            return Some(consumed);
        }
        if let Some(block) = tokenizer::block::leaf(source, &first) {
            if let Some(inline) = block.inline {
                output.scopes.insert(
                    (block.node.range.start, block.node.range.end, false),
                    inline.into(),
                );
            }
            output.push_segmented(source.select(0..block.consumed), block.node);
            return Some(block.consumed);
        }
        if let Some((consumed, node, list)) = self.container(source) {
            output.push_segmented(source.select(0..consumed), node);
            output.list = list;
            return Some(consumed);
        }
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

    /// Merge ordinary code into eligible text using the one dedented operand.
    fn code_block(source: &Segmented, output: &mut BlockTokens) -> Option<usize> {
        let block = tokenizer::block::code(source)?;
        let consumed = block.consumed;
        if !output
            .last()
            .is_some_and(|node| matches!(node.kind, Kind::Paragraph(_) | Kind::Inline(_)))
        {
            output.push_segmented(source.select(0..consumed), block.node);
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
        Self::merge(output, &node, &body.into());
        Some(end)
    }

    /// Merge literal definitions before registering otherwise first-wins references.
    fn definition_block(&mut self, source: &Segmented, output: &mut BlockTokens) -> Option<usize> {
        let matched = rules::definition::recognize(source, self.options)?;
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
            Self::merge(output, node, &body);
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
        Self::merge(output, &node, &body.into());
    }

    /// Extend the semantic tail and transfer its inline scope to the new extent.
    fn merge(output: &mut BlockTokens, node: &Node, body: &Segmented) {
        let Some(last) = output.nodes.last_mut() else {
            return;
        };
        let key = (last.range.start, last.range.end, false);
        let Some(mut previous) = output.scopes.remove(&key) else {
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
        output
            .scopes
            .insert((last.range.start, last.range.end, false), previous);
        output.tail = Tail::Visible(None);
        output.list = None;
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
mod tests;
