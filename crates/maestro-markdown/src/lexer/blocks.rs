//! Retained output and ordered recursive block admission.
use super::Lexer;
use crate::{
    Kind, MarkedOptions, Node, rules,
    source::{ScopeSource, Source},
    tokenizer,
};

/// Effective grammar tail, including an invisible retained definition.
enum Tail {
    /// Last visible block's own raw source when it can reenter a container.
    Visible(Option<ScopeSource>),
    /// A browser definition prevents merges with the preceding visible block.
    Definition,
}

/// One ordered output shared across recursive quote chunks.
#[derive(Default)]
pub(super) struct BlockTokens {
    /// Visible semantic nodes.
    pub(super) nodes: Vec<Node>,
    /// Last grammar token, independent of visible output.
    tail: Tail,
}
impl Default for Tail {
    fn default() -> Self {
        Self::Visible(None)
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
        self.tail = Tail::Visible(
            matches!(node.kind, Kind::List(..) | Kind::Quote(..))
                .then(|| source.select(std::iter::once(0..consumed))),
        );
        self.nodes.push(node);
    }

    /// Borrow the trailing container's raw source for reentry.
    pub(super) fn raw(&mut self) -> Option<ScopeSource> {
        match &mut self.tail {
            Tail::Visible(raw) => raw.take(),
            Tail::Definition => None,
        }
    }
}
impl<E> Lexer<'_, E> {
    /// Append blocks while preserving the invocation's paragraph clipping state.
    pub(super) fn block_tokens(
        &mut self,
        source: &ScopeSource,
        output: &mut BlockTokens,
        mut clipped: bool,
    ) {
        let barriers = self.barriers.take();
        let mut cursor = 0;
        while cursor < source.text.len() && self.error.is_none() {
            let rest = source.view(cursor..source.text.len());
            if let Some(consumed) = self.block(&rest, output) {
                cursor += consumed;
                continue;
            }
            self.clipped = false;
            let Some((consumed, node)) = self.paragraph(&rest) else {
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
                self.merge_node(output, &node);
            } else {
                output.push(&rest, consumed, node);
            }
            clipped = self.clipped;
            cursor += consumed;
        }
        self.top = true;
        self.barriers = barriers;
    }

    /// Admit everything preceding paragraph and line-text fallback.
    fn block(&mut self, source: &Source<'_>, output: &mut BlockTokens) -> Option<usize> {
        if let Some(consumed) = self.extension(source, &mut output.nodes) {
            output.tail = Tail::Visible(None);
            return Some(consumed);
        }
        if let Some(consumed) = self
            .space_block(source, output)
            .or_else(|| self.code_block(source, output))
        {
            return Some(consumed);
        }
        if let Some(block) = tokenizer::block::leaf(source) {
            if let Some(inline) = block.inline {
                self.scopes.insert(
                    (block.node.range.start, block.node.range.end, false),
                    inline,
                );
            }
            output.push(source, block.consumed, block.node);
            return Some(block.consumed);
        }
        if let Some((consumed, node)) = self.container(source) {
            output.push(source, consumed, node);
            return Some(consumed);
        }
        if let Some(block) = tokenizer::block::html(source, self.options) {
            output.push(source, block.consumed, block.node);
            return Some(block.consumed);
        }
        self.definition_block(source, output).or_else(|| {
            self.table(source).map(|(consumed, node)| {
                output.push(source, consumed, node);
                consumed
            })
        })
    }

    /// Consume spacing while keeping the visible tail's raw and scope in sync.
    fn space_block(&mut self, source: &Source<'_>, output: &mut BlockTokens) -> Option<usize> {
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
            let scope = self
                .scopes
                .remove(&(last.range.start, last.range.end, false));
            last.authored.append(&source.authored(0..1));
            last.range.end = source.extent(0..1).end;
            if let Some(scope) = scope {
                self.scopes
                    .insert((last.range.start, last.range.end, false), scope);
            }
        }
        if let Tail::Visible(Some(raw)) = &mut output.tail {
            raw.append(source.select(std::iter::once(0..1)));
        }
        Some(gap.end())
    }

    /// Merge ordinary code into eligible text using the one dedented operand.
    fn code_block(&mut self, source: &Source<'_>, output: &mut BlockTokens) -> Option<usize> {
        let block = tokenizer::block::code(source)?;
        let consumed = if output
            .last()
            .is_some_and(|node| matches!(node.kind, Kind::Paragraph(_) | Kind::Inline(_)))
        {
            let end = code_merge_end(source, block.consumed);
            let body = block.inline?;
            let end_text = body.local(&source.extent(0..end))?.end;
            let end_text = body.text[..end_text].trim_end_matches('\n').len();
            self.merge_source(
                output,
                source,
                end,
                body.select(std::iter::once(0..end_text)),
            );
            end
        } else {
            let consumed = block.consumed;
            output.push(source, consumed, block.node);
            consumed
        };
        Some(consumed)
    }

    /// Merge literal definitions before registering otherwise first-wins references.
    fn definition_block(&mut self, source: &Source<'_>, output: &mut BlockTokens) -> Option<usize> {
        #[cfg(test)]
        crate::tests::block_operand(source.text);
        let matched = rules::definition::recognize(source.text, self.options)?;
        if output
            .last()
            .is_some_and(|node| matches!(node.kind, Kind::Paragraph(_) | Kind::Inline(_)))
        {
            self.merge_source(
                output,
                source,
                matched.consumed,
                source.select(std::iter::once(0..matched.consumed)),
            );
        } else if self.definition(source, &matched)
            && matches!(self.options, MarkedOptions::Browser { .. })
        {
            output.tail = Tail::Definition;
        }
        Some(matched.consumed)
    }

    /// Join inline operands without reconstructing them from bounding node ranges.
    fn merge_node(&mut self, output: &mut BlockTokens, node: &Node) {
        if let Some(body) = self
            .scopes
            .remove(&(node.range.start, node.range.end, false))
        {
            self.merge(output, node, body);
        }
    }

    /// Join a dedented or literal operand while preserving its original provenance.
    fn merge_source(
        &mut self,
        output: &mut BlockTokens,
        source: &Source<'_>,
        consumed: usize,
        body: ScopeSource,
    ) {
        let node = source.node(0..consumed, Kind::Inline(Vec::new()));
        self.merge(output, &node, body);
    }

    /// Extend the semantic tail and transfer its inline scope to the new extent.
    fn merge(&mut self, output: &mut BlockTokens, node: &Node, body: ScopeSource) {
        let Some(last) = output.nodes.last_mut() else {
            return;
        };
        let key = (last.range.start, last.range.end, false);
        let Some(mut previous) = self.scopes.remove(&key) else {
            return;
        };
        if !previous.text.ends_with('\n') {
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
        output.tail = Tail::Visible(None);
    }
}

/// Preserve authored blank lines after merged code, not synthetic reentry seams.
fn code_merge_end(source: &Source<'_>, consumed: usize) -> usize {
    let raw = &source.text[..consumed];
    let mut end = consumed;
    let mut blank = false;
    for range in crate::lexer::lines(raw) {
        let line = &raw[range.clone()];
        if line.trim_matches([' ', '\t', '\n']).is_empty() {
            blank |= !source.extent(range.clone()).is_empty();
        } else {
            end = range.end - usize::from(line.ends_with('\n'));
            blank = false;
        }
    }
    if blank { end } else { consumed }
}
