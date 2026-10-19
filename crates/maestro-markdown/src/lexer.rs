//! Document block dispatch and first-wins reference ownership.
mod quote;
use crate::parse::Extension;
use crate::{Alignment, BlockToken, Cell, Definition, Document, Kind, MarkedOptions, Node, Table};
use crate::{
    helpers,
    inline::ScopeKey,
    rules,
    source::{ScopeSource, Source},
    tokenizer::{self, capture},
};
use maestro_text::is_whitespace_scalar;
use std::{collections::HashMap, ops::Range};

/// Block grammar position, independent of any consumer.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BlockContext {
    /// Paragraphs can be clipped at a block extension start.
    TopLevel,
    /// Tight item text stays inline unless an extension starts the block itself.
    ListItem,
}

/// Per-document grammar state; inline work begins after all definitions are known.
pub(crate) struct Lexer<'a, E> {
    /// Optional caller block grammar, tried before built-in rules.
    tokenizer: Option<&'a mut dyn Extension<E>>,
    /// First invalid extension result, propagated before inline resolution.
    error: Option<E>,
    /// Current paragraph-versus-item grammar position.
    context: BlockContext,
    /// Whether the preceding paragraph was clipped at an extension hint.
    clipped: bool,
    /// Immutable caller grammar.
    pub(crate) options: MarkedOptions,
    /// First definition for each folded identity.
    pub(crate) definitions: HashMap<String, Definition>,
    /// Inline source provenance captured during block recognition.
    pub(crate) scopes: HashMap<ScopeKey, ScopeSource>,
}
impl<'a, E> Lexer<'a, E> {
    /// Parse owned source without borrowing the caller's buffer.
    pub(crate) fn parse(
        source: String,
        options: MarkedOptions,
        tokenizer: Option<&'a mut dyn Extension<E>>,
    ) -> Result<Document, E> {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.reference_indexes += 1);
        let mut lexer = Self {
            tokenizer,
            error: None,
            context: BlockContext::TopLevel,
            clipped: false,
            options,
            definitions: HashMap::new(),
            scopes: HashMap::new(),
        };
        let mut nodes = lexer.blocks(&ScopeSource::identity(&source));
        if let Some(error) = lexer.error {
            return Err(error);
        }
        lexer.resolve(&mut nodes);
        Ok(Document {
            source,
            nodes,
            scopes: lexer.scopes,
            options,
            definitions: lexer.definitions,
        })
    }

    /// Dispatch blocks with a monotonically advancing authored cursor.
    pub(crate) fn blocks(&mut self, source: &ScopeSource) -> Vec<Node> {
        self.blocks_in(source, BlockContext::TopLevel, false)
    }

    /// Preserve item text context while sharing the same block dispatcher.
    pub(crate) fn item_blocks(&mut self, source: &ScopeSource) -> Vec<Node> {
        self.blocks_in(source, BlockContext::ListItem, false)
    }

    /// Restore the enclosing grammar context after recursively admitting containers.
    ///
    /// `open` says the source continues a paragraph that precedes it in the same container.
    fn blocks_in(&mut self, source: &ScopeSource, context: BlockContext, open: bool) -> Vec<Node> {
        let previous_context = std::mem::replace(&mut self.context, context);
        let mut output = Vec::new();
        let mut cursor = 0;
        let mut clipped = open;
        while cursor < source.text.len() {
            let previous = output.len();
            self.clipped = false;
            let rest = source.view(cursor..source.text.len());
            cursor += self.advance(&rest, &mut output, clipped);
            if clipped && output.len() > previous {
                let mut next = output.split_off(previous);
                self.join_continuation(source, &mut output, &mut next);
                output.extend(next);
            }
            clipped = self.clipped;
        }
        self.context = previous_context;
        output
    }

    /// Admit one block and return its positive consumed byte count.
    fn advance(&mut self, source: &Source<'_>, output: &mut Vec<Node>, open: bool) -> usize {
        if self.error.is_some() {
            return source.text.len();
        }
        if let Some(consumed) = self.extension(source, output) {
            return consumed;
        }
        let gap = rules::BLOCK_NEWLINE
            .as_ref()
            .and_then(|rule| rule.find(source.text));
        if let Some(gap) = gap.filter(|gap| gap.end() > 0) {
            if (gap.end() > 1
                && (!output.is_empty()
                    || source.text[gap.end()..]
                        .chars()
                        .any(|scalar| !is_whitespace_scalar(scalar))))
                || output
                    .last()
                    .is_some_and(|node| node.authored.ends_with('\n'))
            {
                output.push(source.node(gap.range(), Kind::Gap));
            }
            return gap.end();
        }
        let continued = open
            .then(|| tokenizer::block::continuation(source, self.options))
            .flatten();
        self.clipped = continued.as_ref().is_some_and(|(_, open)| *open);
        if let Some(block) = continued
            .map(|(block, _)| block)
            .or_else(|| tokenizer::block::leaf(source, self.options))
        {
            if let Some(inline) = block.inline {
                self.scopes.insert(
                    (block.node.range.start, block.node.range.end, false),
                    inline,
                );
            }
            output.push(block.node);
            return block.consumed;
        }
        if let Some((consumed, node)) = self.container(source) {
            self.clipped = false;
            output.push(node);
            return consumed;
        }
        if let Some(consumed) = self.definition(source) {
            return consumed;
        }
        if let Some((consumed, node)) = self.table(source).or_else(|| self.paragraph(source)) {
            output.push(node);
            return consumed;
        }
        let size = source.text.chars().next().map_or(0, char::len_utf8);
        let node = source.node(0..size, Kind::Paragraph(Vec::new()));
        self.scopes.insert(
            (node.range.start, node.range.end, false),
            source.select(std::iter::once(0..size)),
        );
        output.push(node);
        size
    }

    /// Try a validated caller block extension before built-in tokenizers.
    fn extension(&mut self, source: &Source<'_>, output: &mut Vec<Node>) -> Option<usize> {
        let token = match self.tokenizer.as_mut()?.tokenize(source.text) {
            Ok(token) => token?,
            Err(error) => {
                self.error = Some(error);
                return Some(source.text.len());
            }
        };
        let BlockToken { raw, kind, text } = token;
        let consumed = raw.len();
        output.push(Node {
            range: source.extent(0..consumed),
            authored: raw,
            kind: Kind::CustomBlock(kind, text),
        });
        Some(consumed)
    }

    /// Keep paragraph text before the next possible caller-owned block.
    fn extension_start(&mut self, source: &str) -> usize {
        if self.context == BlockContext::ListItem || self.tokenizer.is_none() {
            return source.len();
        }
        let mut barriers = None;
        let mut begin = source.chars().next().map_or(0, char::len_utf8);
        while begin < source.len() {
            let Some(tokenizer) = self.tokenizer.as_mut() else {
                break;
            };
            let start = match tokenizer.start(&source[begin..]) {
                Ok(Some(start)) => begin + start,
                Ok(None) => break,
                Err(error) => {
                    self.error = Some(error);
                    break;
                }
            };
            let barriers =
                barriers.get_or_insert_with(|| tokenizer::inline::barriers(source, self.options));
            let boundary = barriers.partition_point(|range| range.end <= start);
            if let Some(range) = barriers
                .get(boundary)
                .filter(|range| range.contains(&start))
            {
                begin = range.end;
            } else {
                self.clipped = true;
                return start;
            }
        }
        source.len()
    }

    /// Recognize quote/list containers before definitions and ordinary paragraphs.
    fn container(&mut self, source: &Source<'_>) -> Option<(usize, Node)> {
        self.quote(source).or_else(|| self.list(source))
    }

    /// Capture definitions once, preserving the first destination and title.
    fn definition(&mut self, source: &Source<'_>) -> Option<usize> {
        let matched = rules::block_def(self.options)?.find(source.text)?;
        let id = reference_key(capture(source.text, &matched, 1));
        let destination = capture(source.text, &matched, 2);
        let destination = destination
            .strip_prefix('<')
            .and_then(|text| text.strip_suffix('>'))
            .unwrap_or(destination);
        let title = capture(source.text, &matched, 3);
        let title = title.get(1..title.len().saturating_sub(1)).unwrap_or("");
        self.definitions.entry(id).or_insert_with(|| Definition {
            destination: destination.to_owned(),
            title: title.to_owned(),
        });
        Some(matched.end())
    }

    /// Capture a setext heading or an ordinary paragraph's inline source.
    fn paragraph(&mut self, source: &Source<'_>) -> Option<(usize, Node)> {
        let heading = rules::block_lheading(self.options).and_then(|rule| rule.find(source.text));
        let limit = if heading.is_none() {
            self.extension_start(source.text)
        } else {
            source.text.len()
        };
        let window = source.prefix(limit);
        let (matched, kind) = if let Some(matched) = heading {
            let level = if capture(source.text, &matched, 2).starts_with('=') {
                1
            } else {
                2
            };
            let label = capture(source.text, &matched, 1).to_owned();
            (matched, Kind::Heading(level, Vec::new(), label))
        } else {
            (
                rules::block_paragraph(self.options)?.find(window.text)?,
                Kind::Paragraph(Vec::new()),
            )
        };
        let range = matched.group(1)?;
        let logical_end = if matches!(kind, Kind::Paragraph(_)) {
            paragraph_end(&window, matched.end())
        } else {
            matched.end()
        };
        let end = logical_end
            + usize::from(
                matches!(kind, Kind::Paragraph(_))
                    && source.text.as_bytes().get(logical_end) == Some(&b'\n'),
            );

        let node = source.node(0..end, kind);
        let inline_range = if matches!(node.kind, Kind::Paragraph(_)) {
            range.start..end
        } else {
            range
        };
        self.scopes.insert(
            (node.range.start, node.range.end, false),
            source.select(std::iter::once(inline_range)),
        );
        Some((end, node))
    }

    /// Select a table only when header and delimiter column counts agree.
    fn table(&mut self, source: &Source<'_>) -> Option<(usize, Node)> {
        if self.options == MarkedOptions::Documentation || source.is_lazy(0) {
            return None;
        }
        let matched = rules::block_table(self.options)?.find(source.text)?;
        let delimiter = capture(source.text, &matched, 2);
        if !delimiter.contains(['|', ':']) {
            return None;
        }
        let headers = helpers::cell_ranges(capture(source.text, &matched, 1));
        let alignments: Vec<_> = helpers::split_cells(delimiter, None)
            .into_iter()
            .map(|cell| match (cell.starts_with(':'), cell.ends_with(':')) {
                (true, true) => Alignment::Center,
                (true, false) => Alignment::Left,
                (false, true) => Alignment::Right,
                (false, false) => Alignment::None,
            })
            .collect();
        if headers.len() != alignments.len() {
            return None;
        }
        let count = headers.len();
        let header = self.cells(source, matched.group(1)?, count);
        let rows = matched
            .group(3)
            .into_iter()
            .flat_map(|range| {
                lines(&source.text[range.clone()])
                    .map(move |line| range.start + line.start..range.start + line.end)
            })
            .filter(|range| {
                !source.text[range.clone()]
                    .trim_matches(is_whitespace_scalar)
                    .is_empty()
            })
            .map(|range| self.cells(source, range, count))
            .collect();
        let table = Table {
            header,
            rows,
            alignments,
            authored: helpers::rtrim(&source.text[matched.range()], '\n').to_owned(),
        };
        Some((
            matched.end(),
            source.node(matched.range(), Kind::Table(table)),
        ))
    }

    /// Retain padded authored cell scopes but interpret their trimmed content.
    fn cells(&mut self, source: &Source<'_>, row: Range<usize>, count: usize) -> Vec<Cell> {
        let ranges = helpers::cell_ranges(source.text[row.clone()].trim_end_matches('\n'));
        (0..count)
            .map(|index| {
                let range = ranges.get(index).map_or(row.end..row.end, |range| {
                    row.start + range.start..row.start + range.end
                });
                let input = source.select(std::iter::once(range.clone())).pipes();
                let range = source.extent(range);
                let authored = input.text.clone();
                self.scopes.insert((range.start, range.end, false), input);
                Cell {
                    children: Vec::new(),
                    authored,
                    range,
                }
            })
            .collect()
    }

    /// Resolve inline bodies after nested containers supply their definitions.
    fn resolve(&mut self, nodes: &mut [Node]) {
        for node in nodes {
            self.resolve_node(node);
        }
    }

    /// Resolve one retained container without rebuilding its document.
    fn resolve_node(&mut self, node: &mut Node) {
        match &mut node.kind {
            Kind::Paragraph(children) | Kind::Inline(children) | Kind::Heading(_, children, _) => {
                self.resolve_body(&node.range, children, false);
            }
            Kind::Quote(children, _) | Kind::List(_, children) | Kind::Item(children) => {
                self.resolve(children);
            }
            Kind::Table(table) => self.resolve_table(table),
            _ => {}
        }
    }

    /// Resolve each cell against its own padded original-source scope.
    fn resolve_table(&mut self, table: &mut Table) {
        for cell in table
            .header
            .iter_mut()
            .chain(table.rows.iter_mut().flatten())
        {
            self.resolve_body(&cell.range, &mut cell.children, true);
        }
    }

    /// Interpret an indexed inline body, keeping the complete source for later selection.
    fn resolve_body(&mut self, range: &Range<usize>, children: &mut Vec<Node>, cell: bool) {
        let key = (range.start, range.end, false);
        let Some(source) = self.scopes.remove(&key) else {
            return;
        };
        let text = if cell {
            source.text.trim_matches(is_whitespace_scalar)
        } else {
            source.text.strip_suffix('\n').unwrap_or(&source.text)
        };
        let start = source.text.find(text).unwrap_or(0);
        *children = self.inline(&source.select(std::iter::once(start..start + text.len())));
        self.scopes.insert(key, source);
    }

    /// Invoke the shared inline grammar using the document's retained definitions.
    fn inline(&mut self, source: &ScopeSource) -> Vec<Node> {
        tokenizer::inline::Tokenizer::new(
            source,
            self.options,
            &self.definitions,
            Some(&mut self.scopes),
        )
        .parse(&[])
    }
}

/// Fold reference whitespace without losing absent or empty destinations.
pub(crate) fn reference_id(source: &str) -> String {
    source
        .split(is_whitespace_scalar)
        .filter(|piece| !piece.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Source ranges of complete physical lines, including each terminator.
pub(crate) fn lines(source: &str) -> impl Iterator<Item = Range<usize>> {
    let mut offset = 0;
    source.split_inclusive('\n').map(move |line| {
        let start = offset;
        offset += line.len();
        start..offset
    })
}

/// Match reference identities using full Unicode folding without changing their spelling.
pub(crate) fn reference_key(source: &str) -> String {
    unicase::UniCase::new(reference_id(source)).to_folded_case()
}

/// Keep a lazy table-looking continuation in its original paragraph.
fn paragraph_end(source: &Source<'_>, mut end: usize) -> usize {
    while source.text.as_bytes().get(end) == Some(&b'\n') {
        let next = end + 1;
        if !source.is_lazy(next) {
            break;
        }
        let Some(table) = rules::BLOCK_TABLE.as_ref() else {
            break;
        };
        let Some(paragraph) = rules::BLOCK_PARAGRAPH.as_ref() else {
            break;
        };
        if table.find(&source.text[next..]).is_none() {
            break;
        }
        let Some(continuation) = paragraph.find(&source.text[next..]) else {
            break;
        };
        end = next + continuation.end();
    }
    end
}
