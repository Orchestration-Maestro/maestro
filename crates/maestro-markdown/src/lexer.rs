//! Document block dispatch and first-wins reference ownership.
mod blocks;
mod frames;
mod quote;
use crate::parse::Extension;
use crate::{Alignment, BlockToken, Cell, Definition, Document, Kind, MarkedOptions, Node, Table};
use crate::{
    helpers,
    inline::ScopeKey,
    rules,
    source::{ScopeSource, Source},
    tokenizer,
};
pub(crate) use blocks::BlockTokens;
use maestro_text::is_whitespace_scalar;
use std::{
    collections::{HashMap, hash_map::Entry},
    ops::Range,
};

/// Per-document grammar state; inline work begins after all definitions are known.
pub(crate) struct Lexer<'a, E> {
    /// Optional caller block grammar, tried before built-in rules.
    tokenizer: Option<&'a mut dyn Extension<E>>,
    /// First invalid extension result, propagated before inline resolution.
    error: Option<E>,
    /// Current paragraph-versus-item grammar position.
    top: bool,
    /// Whether the preceding paragraph was clipped at an extension hint.
    clipped: bool,
    /// Link context retained across the document's queued inline bodies.
    context: tokenizer::inline::LinkContext,
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
            top: true,
            clipped: false,
            context: tokenizer::inline::LinkContext::new(false),
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

    /// Admit a document into one retained block accumulator.
    pub(crate) fn blocks(&mut self, source: &ScopeSource) -> Vec<Node> {
        let mut output = BlockTokens::default();
        self.block_tokens(source, &mut output, false);
        self.finish_blocks(output)
    }

    /// Replay item admission without publishing its inline operands prematurely.
    pub(crate) fn item_blocks(&mut self, source: &ScopeSource, output: &mut BlockTokens) {
        output.complete();
        for node in &output.nodes {
            self.forget(node);
        }
        *output = BlockTokens::default();
        self.top = false;
        self.block_tokens(source, output, false);
    }

    /// Try a validated caller block extension before built-in tokenizers.
    fn extension(&mut self, source: &Source<'_>, output: &mut BlockTokens) -> Option<usize> {
        let token = match self.tokenizer.as_mut()?.tokenize(source.text) {
            Ok(token) => token?,
            Err(error) => {
                self.error = Some(error);
                return Some(source.text.len());
            }
        };
        let BlockToken { raw, kind, text } = token;
        let consumed = raw.len();
        output.push(
            source,
            consumed,
            source.node(0..consumed, Kind::CustomBlock(kind, text)),
        );
        Some(consumed)
    }

    /// Keep paragraph text before the next possible caller-owned block; `source` runs to the
    /// end of its block source.
    fn extension_start(&mut self, source: &Source<'_>) -> usize {
        let source = source.text;
        let begin = source.chars().next().map_or(0, char::len_utf8);
        let Some(tokenizer) = self.tokenizer.as_mut() else {
            return source.len();
        };
        match tokenizer.start(&source[begin..]) {
            Ok(Some(start)) => {
                let start = begin + start;
                self.clipped = start < source.len();
                start
            }
            Ok(None) => source.len(),
            Err(error) => {
                self.error = Some(error);
                source.len()
            }
        }
    }

    /// Recognize quote/list containers before definitions and ordinary paragraphs.
    fn container(
        &mut self,
        source: &Source<'_>,
    ) -> Option<(usize, Node, Option<Box<tokenizer::block::ListState>>)> {
        self.quote(source)
            .map(|(end, node)| (end, node, None))
            .or_else(|| {
                self.list(source)
                    .map(|(end, node, state)| (end, node, Some(state)))
            })
    }

    /// Register a recognized definition only once, reporting whether it is new.
    fn definition(&mut self, source: &Source<'_>, matched: &rules::definition::Definition) -> bool {
        let id = reference_key(&source.text[matched.label.clone()]);
        let destination = &source.text[matched.destination.clone()];
        let destination = destination
            .strip_prefix('<')
            .and_then(|text| text.strip_suffix('>'))
            .unwrap_or(destination);
        let title = matched
            .title
            .as_ref()
            .map_or("", |range| &source.text[range.clone()]);
        let title = title.get(1..title.len().saturating_sub(1)).unwrap_or("");
        let Entry::Vacant(entry) = self.definitions.entry(id) else {
            return false;
        };
        entry.insert(Definition {
            destination: destination.to_owned(),
            title: title.to_owned(),
        });
        true
    }

    /// Capture setext headings before selecting top-level paragraphs or item text.
    fn paragraph(
        &mut self,
        source: &Source<'_>,
        output: &mut BlockTokens,
    ) -> Option<(usize, Node)> {
        let recognized = rules::paragraph::Recognizer::new(self.options).finish(source.text);
        let limit = if recognized.heading.is_none() {
            self.extension_start(source)
        } else {
            source.text.len()
        };
        let (end, content, kind) = if let Some(heading) = recognized.heading {
            let label = source.text[heading.content.clone()].to_owned();
            (
                heading.consumed,
                heading.content,
                Kind::Heading(heading.level, Vec::new(), label),
            )
        } else if self.top {
            let end = if limit == source.text.len() {
                recognized.paragraph?
            } else {
                rules::paragraph::Recognizer::new(self.options)
                    .finish(&source.text[..limit])
                    .paragraph?
            };
            (end, 0..end, Kind::Paragraph(Vec::new()))
        } else {
            let end = source.text.find('\n').unwrap_or(source.text.len());
            (end, 0..end, Kind::Inline(Vec::new()))
        };
        let node = source.node(0..end, kind);
        output.scopes.insert(
            (node.range.start, node.range.end, false),
            source.select(std::iter::once(content)).into(),
        );
        Some((end, node))
    }

    /// Select a table only when header and delimiter column counts agree.
    fn table(&mut self, source: &Source<'_>, output: &mut BlockTokens) -> Option<(usize, Node)> {
        if self.options == MarkedOptions::Documentation {
            return None;
        }
        let matched = rules::table::Recognizer::new(self.options).finish(source.text)?;
        let delimiter = &source.text[matched.delimiter];
        if !delimiter.contains(['|', ':']) {
            return None;
        }
        let headers = helpers::cell_ranges(&source.text[matched.header.clone()]);
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
        let header = Self::cells(source, matched.header, count, output);
        let rows = matched
            .body
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
            .map(|range| Self::cells(source, range, count, output))
            .collect();
        let table = Table {
            header,
            rows,
            alignments,
            authored: helpers::rtrim(&source.text[..matched.consumed], '\n').to_owned(),
        };
        Some((
            matched.consumed,
            source.node(0..matched.consumed, Kind::Table(table)),
        ))
    }

    /// Retain padded authored cell scopes but interpret their trimmed content.
    fn cells(
        source: &Source<'_>,
        row: Range<usize>,
        count: usize,
        output: &mut BlockTokens,
    ) -> Vec<Cell> {
        let ranges = helpers::cell_ranges(source.text[row.clone()].trim_end_matches('\n'));
        (0..count)
            .map(|index| {
                let range = ranges.get(index).map_or(row.end..row.end, |range| {
                    row.start + range.start..row.start + range.end
                });
                let input = source.select(std::iter::once(range.clone())).pipes();
                let range = source.extent(range);
                output
                    .scopes
                    .insert((range.start, range.end, false), input.into());
                Cell {
                    children: Vec::new(),
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
        let mut tokenizer = tokenizer::inline::Tokenizer::new(
            source,
            self.options,
            &self.definitions,
            Some(&mut self.scopes),
            self.context.active(),
        );
        let nodes = tokenizer.parse(&[], None);
        self.context = tokenizer.context;
        nodes
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
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.framing_bytes += line.len());
        let start = offset;
        offset += line.len();
        start..offset
    })
}

/// Match reference identities using full Unicode folding without changing their spelling.
pub(crate) fn reference_key(source: &str) -> String {
    unicase::UniCase::new(reference_id(source)).to_folded_case()
}
