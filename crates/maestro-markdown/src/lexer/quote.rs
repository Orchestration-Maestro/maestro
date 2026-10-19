//! Quoted chunks and nested continuation re-lexing.
use super::{BlockContext, Lexer, lines};
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
        let mut children: Vec<Node> = Vec::new();
        let mut cursor = 0;
        while cursor < limit {
            let end = quote_chunk(&source.text[cursor..limit]) + cursor;
            let local = body.local(&source.extent(cursor..end))?;
            let chunk = body.select(std::iter::once(local));
            let open = children
                .last()
                .is_some_and(|node| matches!(node.kind, Kind::Paragraph(_)));
            let mut next = self.blocks_in(&chunk, BlockContext::TopLevel, open);
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
                // Continue the container with its own text and only the lines no chunk consumed.
                let end =
                    local.end + usize::from(body.text.as_bytes().get(local.end) == Some(&b'\n'));
                let mut input = body.select(std::iter::once(local.start..end));
                input.append(source.select(std::iter::once(cursor..limit)));
                let (_, node) = self.container(&input.view(0..input.text.len()))?;
                cursor = cursor.max(source.local(&node.range)?.end);
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
        let left = self
            .scopes
            .remove(&(last.range.start, last.range.end, false));
        let right = self
            .scopes
            .remove(&(first.range.start, first.range.end, false));
        let between = source.local(&(last.range.end..first.range.start));
        last.range.end = first.range.end;
        let (Some(mut merged), Some(right), Some(between), Some(local)) =
            (left, right, between, source.local(&last.range))
        else {
            return;
        };
        // Inline text joins both paragraphs' own text, which a continuation may have respelled.
        merged.append(source.select(std::iter::once(between)));
        merged.append(right);
        last.authored = source.select(std::iter::once(local)).text;
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
///
/// A lazy setext underline after another lazy line is indented to four spaces, so it stays
/// paragraph text instead of turning the lazy lines into a heading.
fn quote_source(source: &Source<'_>, end: usize) -> ScopeSource {
    let mut pieces = Vec::new();
    let mut lazy = Vec::new();
    let mut underlines = Vec::new();
    let mut local = 0;
    let mut previous_lazy = false;
    for range in lines(&source.text[..end]) {
        let line = &source.text[range.clone()];
        let shown = crate::source::strip_quote(line);
        let is_lazy = shown.len() == line.len();
        if is_lazy {
            if previous_lazy && let Some(spaces) = underline(line) {
                let marker = &line[spaces..=spaces];
                underlines.push((local..local + spaces + 1, format!("    {marker}")));
            }
            lazy.push(local);
        }
        previous_lazy = is_lazy;
        pieces.push(range.start + line.len() - shown.len()..range.end);
        local += shown.len();
    }
    let mut body = source.select(pieces);
    body.lazy_lines = lazy;
    body.respell(underlines)
}

/// Leading spaces of a setext underline line (up to three, then `=` or `-` and spaces).
fn underline(line: &str) -> Option<usize> {
    let line = line.strip_suffix('\n').unwrap_or(line);
    let spaces = line.bytes().take_while(|byte| *byte == b' ').count();
    let marker = *line
        .as_bytes()
        .get(spaces)
        .filter(|byte| matches!(byte, b'=' | b'-'))?;
    let rest = line[spaces..].trim_start_matches(char::from(marker));
    (spaces <= 3 && rest.bytes().all(|byte| byte == b' ')).then_some(spaces)
}
