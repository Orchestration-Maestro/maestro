//! Quoted chunks and nested continuation re-lexing.
use super::{Lexer, lines};
use crate::{
    Kind, Node, helpers, rules,
    source::{ScopeSource, Source},
};
impl<E> Lexer<'_, E> {
    /// Admit quoted and continuation chunks using their original source coordinates.
    pub(super) fn quote(&mut self, source: &Source<'_>) -> Option<(usize, Node)> {
        let matched = rules::block_blockquote(self.options)?.find(source.text)?;
        let limit =
            (helpers::rtrim(&source.text[matched.range()], '\n').len() + 1).min(matched.end());
        let body = quote_source(source, limit);
        let mut children = Vec::new();
        let mut cursor = 0;
        while cursor < limit {
            let end = quote_chunk(&source.text[cursor..limit]) + cursor;
            let local = body.local(&source.extent(cursor..end))?;
            let chunk = body.select(std::iter::once(local));
            let mut next = self.blocks(&chunk);
            self.join_continuation(&body, &mut children, &mut next);
            children.extend(next);
            cursor = end;
            if cursor == limit
                || children
                    .last()
                    .is_some_and(|node| matches!(node.kind, Kind::CodeBlock(..)))
            {
                break;
            }
            if children
                .last()
                .is_some_and(|node| matches!(node.kind, Kind::Quote(..) | Kind::List(..)))
            {
                let old = children.pop()?;
                let local = body.local(&old.range)?;
                self.forget(&old);
                let (_, node) = self.container(&body.view(local.start..body.text.len()))?;
                cursor = source.local(&node.range)?.end;
                cursor += usize::from(source.text.as_bytes().get(cursor) == Some(&b'\n'));
                children.push(node);
            }
            if children
                .last()
                .is_some_and(|node| matches!(node.kind, Kind::Quote(..)))
            {
                break;
            }
        }
        let authored = body
            .select(std::iter::once(
                0..body.local(&source.extent(0..cursor))?.end,
            ))
            .text;
        Some((
            cursor,
            source.node(0..cursor, Kind::Quote(children, authored)),
        ))
    }

    /// Merge the first paragraph of a continuation before admitting inline operands.
    pub(super) fn join_continuation(
        &mut self,
        source: &ScopeSource,
        previous: &mut [Node],
        next: &mut Vec<Node>,
    ) {
        let Some(last) = previous
            .last_mut()
            .filter(|node| matches!(node.kind, Kind::Paragraph(_)))
        else {
            return;
        };
        if !next
            .first()
            .is_some_and(|node| matches!(node.kind, Kind::Paragraph(_)))
        {
            return;
        }
        let first = next.remove(0);
        self.scopes
            .remove(&(last.range.start, last.range.end, false));
        self.scopes
            .remove(&(first.range.start, first.range.end, false));
        last.range.end = first.range.end;
        let Some(local) = source.local(&last.range) else {
            return;
        };
        let merged = source.select(std::iter::once(local));
        last.authored.clone_from(&merged.text);
        self.scopes
            .insert((last.range.start, last.range.end, false), merged);
    }

    /// Release source frames belonging to the container being replaced by its continuation.
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
/// Select one quoted run together with any preceding lazy continuation lines.
fn quote_chunk(source: &str) -> usize {
    let mut quoted = false;
    for range in lines(source) {
        let line = &source[range.clone()];
        if crate::source::strip_quote(line).len() != line.len() {
            quoted = true;
        } else if quoted {
            return range.start;
        }
    }
    source.len()
}

/// Strip one quote prefix while keeping every line's native provenance.
fn quote_source(source: &Source<'_>, end: usize) -> ScopeSource {
    let mut pieces = Vec::new();
    let mut lazy = Vec::new();
    let mut local = 0;
    for range in lines(&source.text[..end]) {
        let line = &source.text[range.clone()];
        let shown = crate::source::strip_quote(line);
        if shown.len() == line.len() {
            lazy.push(local);
        }
        pieces.push(range.start + line.len() - shown.len()..range.end);
        local += shown.len();
    }
    let mut body = source.select(pieces);
    body.lazy_lines = lazy;
    body
}
