//! Quoted chunks and raw-source container reentry.
use super::{Lexer, blocks::BlockTokens, lines};
use crate::{
    Kind, Node, helpers, rules,
    source::{ScopeSource, Source},
};

impl<E> Lexer<'_, E> {
    /// Recognize raw framing independently of the transformed quote body.
    pub(super) fn quote(&mut self, source: &Source<'_>) -> Option<(usize, Node)> {
        let matched = rules::block_blockquote(self.options)?.find(source.text)?;
        let limit = helpers::rtrim(&source.text[matched.range()], '\n').len();
        let mut pending = source.select(std::iter::once(0..limit));
        let mut cursor = 0;
        let mut text = source.select(std::iter::empty());
        let mut output = BlockTokens::default();
        let mut consumed = 0;
        loop {
            let rest = pending.view(cursor..pending.text.len());
            let end = quote_chunk(rest.text);
            let chunk_end = end - usize::from(end < rest.text.len());
            let current = rest.select(std::iter::once(0..chunk_end));
            cursor += end;
            consumed = consumed.max(source.local(&current.extent(0..current.text.len()))?.end);
            let chunk = quote_source(&current);
            if !text.text.is_empty() {
                text.newline();
            }
            let top = self.top;
            self.top = true;
            self.block_tokens(&chunk, &mut output, true);
            self.top = top;
            text.append(chunk);
            if cursor == pending.text.len()
                || output
                    .last()
                    .is_some_and(|node| matches!(node.kind, Kind::CodeBlock(..)))
            {
                break;
            }
            if !output
                .last()
                .is_some_and(|node| matches!(node.kind, Kind::List(..) | Kind::Quote(..)))
            {
                continue;
            }
            let quote = output
                .last()
                .is_some_and(|node| matches!(node.kind, Kind::Quote(..)));
            let remaining = pending.select(std::iter::once(cursor..pending.text.len()));
            let (used, rest) = self.reenter(&mut output, remaining, &mut text)?;
            let end = source.local(&used)?.end;
            #[cfg(test)]
            crate::tests::WORK.with_borrow_mut(|work| {
                work.reentry_end = end;
                work.reentry_previous = consumed;
            });
            consumed = consumed.max(end);
            pending = rest;
            cursor = 0;
            if quote || pending.text.is_empty() {
                break;
            }
        }
        Some((
            consumed,
            source.node(0..consumed, Kind::Quote(output.nodes, text.text.mapped())),
        ))
    }

    /// Replace a trailing container using only its own raw and the unconsumed input.
    fn reenter(
        &mut self,
        output: &mut BlockTokens,
        remaining: ScopeSource,
        text: &mut ScopeSource,
    ) -> Option<(std::ops::Range<usize>, ScopeSource)> {
        let old = output.nodes.pop()?;
        let mut joined = output.raw()?;
        let prefix = match &old.kind {
            Kind::Quote(_, body) => text.text.len().checked_sub(body.len())?,
            _ => text.local(&joined.extent(0..joined.text.len()))?.start,
        };
        joined.newline();
        joined.append(remaining);
        self.forget(&old);
        let view = joined.view(0..joined.text.len());
        let (consumed, node) = if matches!(old.kind, Kind::Quote(..)) {
            self.quote(&view)?
        } else {
            self.list(&view)?
        };
        let used = view.select(std::iter::once(0..consumed));
        text.truncate(prefix);
        let extent = used.extent(0..used.text.len());
        if let Kind::Quote(_, body) = &node.kind {
            text.append(used.replace(&used.text, &body.to_owned_string()));
        } else {
            text.append(used);
        }
        output.push(&view, consumed, node);
        Some((
            extent,
            view.select(std::iter::once(consumed..view.text.len())),
        ))
    }

    /// Release inline scopes of a replaced container before recursive admission.
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

/// Select one quoted run with its preceding lazy lines and following separator.
fn quote_chunk(source: &str) -> usize {
    let mut quoted = false;
    for range in lines(source) {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.quote_lines += 1);
        let line = &source[range.clone()];
        if crate::source::strip_quote(line).len() != line.len() {
            quoted = true;
        } else if quoted {
            return range.start;
        }
    }
    source.len()
}

/// Rewrite unquoted setext spelling within this chunk before removing prefixes.
fn quote_source(source: &ScopeSource) -> ScopeSource {
    let mut output = source.select(std::iter::empty());
    for range in lines(&source.text) {
        let line = &source.text[range.clone()];
        let shown = crate::source::strip_quote(line);
        let mut piece = source.select(std::iter::once(
            range.start + line.len() - shown.len()..range.end,
        ));
        let spaces = line.bytes().take_while(|byte| *byte == b' ').count();
        let label = line[spaces..].trim_end_matches([' ', '\n']);
        if range.start > 0
            && spaces <= 3
            && !label.is_empty()
            && (label.bytes().all(|byte| byte == b'=') || label.bytes().all(|byte| byte == b'-'))
        {
            piece = piece.replace_prefix(spaces, "    ");
        }
        output.append(piece);
    }
    output
}
