//! Inline precedence over original source and typed atom boundaries.
mod emphasis;
mod entities;
mod links;
use super::{capture, node};
use crate::{
    Definition, Kind, MarkedOptions, Node, Text, inline::ScopeKey, rules, source::ScopeSource,
};
pub(crate) use entities::decode_operand;
use std::{collections::HashMap, ops::Range};

/// One inline invocation; original definitions and policy are never reconstructed.
pub(crate) struct Tokenizer<'a> {
    /// Framed authored source.
    pub(crate) source: &'a ScopeSource,
    /// Immutable grammar selection.
    pub(crate) options: MarkedOptions,
    /// Original document definitions.
    pub(crate) definitions: &'a HashMap<String, Definition>,
    /// Optional document scope collector, absent during local interpretation.
    pub(crate) scopes: Option<&'a mut HashMap<ScopeKey, ScopeSource>>,
    /// Original operand identities for local interpretation, indexed like the atom ranges.
    pub(crate) retained_atoms: Option<&'a [crate::InlineAtom<'a>]>,
    /// Raw anchor state shared by recursive inline spans.
    raw_anchor: bool,
    /// Local interpretation has no document scope collector.
    #[cfg(test)]
    local: bool,
}
impl<'a> Tokenizer<'a> {
    /// Bind one source view to its document state.
    pub(crate) fn new(
        source: &'a ScopeSource,
        options: MarkedOptions,
        definitions: &'a HashMap<String, Definition>,
        scopes: Option<&'a mut HashMap<ScopeKey, ScopeSource>>,
    ) -> Self {
        #[cfg(test)]
        let local = scopes.is_none();
        Self {
            source,
            options,
            definitions,
            scopes,
            raw_anchor: false,
            retained_atoms: None,
            #[cfg(test)]
            local,
        }
    }

    /// Admit inline constructs with typed atoms ahead of ordinary grammar.
    pub(crate) fn parse(&mut self, atoms: &[Range<usize>]) -> Vec<Node> {
        self.tokens(0..self.source.text.len(), atoms, false)
    }

    /// Interpret one inline span without admitting block constructs.
    pub(crate) fn tokens(
        &mut self,
        extent: Range<usize>,
        atoms: &[Range<usize>],
        in_link: bool,
    ) -> Vec<Node> {
        #[cfg(test)]
        if self.local {
            crate::tests::WORK.with_borrow_mut(|work| work.local_bytes += extent.len());
        }
        let mut output = Vec::new();
        let mut plain_tail = None;
        let mut cursor = extent.start;
        let mut line_end = cursor;
        // Only a text run keeps the previous character; a run ending in '_' keeps the earlier one.
        let mut kept = None;
        let masked = self.mask(extent.clone(), atoms);
        while cursor < extent.end {
            let previous = kept.take();
            let atom = atoms.partition_point(|range| range.end <= cursor);
            if let Some(range) = atoms.get(atom).filter(|range| range.start == cursor) {
                self.push_atom(range.clone(), atom, &mut output);
                cursor = range.end;
                continue;
            }
            let limit = atoms
                .get(atom)
                .map_or(extent.end, |range| range.start.min(extent.end));
            let text = &self.source.text[cursor..extent.end];
            let token = self
                .soft_break(text)
                .or_else(|| self.admit_operand(cursor..extent.end, atoms, in_link))
                .or_else(|| {
                    let masked = (masked.as_str(), extent.start, previous);
                    self.emphasis(cursor..extent.end, masked, atoms, in_link)
                })
                .or_else(|| self.line_break(text))
                .or_else(|| self.deletion(cursor..extent.end, atoms, in_link));
            if let Some((size, kind)) =
                token.filter(|(size, kind)| *size > 0 && fits(atoms, cursor + size, limit, kind))
            {
                self.track_anchor(&kind);
                output.push(node(self.source, cursor..cursor + size, kind));
                cursor += size;
                continue;
            }
            let address = self.address(text, &output, in_link);
            if let Some(size) = address.filter(|size| cursor + size <= limit) {
                self.push_text(cursor..cursor + size, &mut output, &mut plain_tail);
                cursor += size;
                continue;
            }
            if cursor >= line_end {
                line_end = self.line_end(cursor..extent.end);
            }
            // Only the first line is kept, so the recognizer never reads past its newline.
            let line = &text[..line_end.min(limit) - cursor];
            let size = self.text(line);
            let size = line[..size].find('\n').unwrap_or(size);
            self.push_text(cursor..cursor + size, &mut output, &mut plain_tail);
            kept = line[..size]
                .chars()
                .next_back()
                .filter(|&c| c != '_')
                .or(previous);
            cursor += size;
        }
        output
    }

    /// Retain operand identity and apply raw HTML transitions, never caller-opaque contents.
    fn push_atom(&mut self, range: Range<usize>, index: usize, output: &mut Vec<Node>) {
        if let Some(crate::InlineAtom::Resolved(original)) =
            self.retained_atoms.and_then(|atoms| atoms.get(index))
        {
            self.track_anchor(&original.kind);
        }
        output.push(node(self.source, range, Kind::Atom(index)));
    }

    /// Update the shared raw-anchor state only after a tag has been admitted.
    fn track_anchor(&mut self, kind: &Kind) {
        if let Kind::Html(html) = kind {
            self.raw_anchor = crate::autolinks::anchor_state(html, self.raw_anchor);
        }
    }

    /// Keep accepted bare addresses whole where the grammar enables URL tokens.
    fn address(&self, text: &str, output: &[Node], in_link: bool) -> Option<usize> {
        let previous = match output.last().map(|node| &node.kind) {
            Some(Kind::Text(text)) => text.authored.chars().next_back(),
            _ => None,
        };
        (!in_link && !self.raw_anchor && self.options != MarkedOptions::Documentation)
            .then(|| crate::autolinks::address(text, previous, self.options))
            .flatten()
    }

    /// Emit decoded units with their original source boundaries.
    fn push_text(
        &self,
        range: Range<usize>,
        output: &mut Vec<Node>,
        plain_tail: &mut Option<usize>,
    ) {
        entities::decode_units(&self.source.text[range.clone()], |unit, decoded| {
            let range = range.start + unit.start..range.start + unit.end;
            append_text(
                output,
                node(
                    self.source,
                    range.clone(),
                    Kind::Text(Text {
                        authored: self.source.text[range].to_owned(),
                        decoded: decoded.to_owned(),
                    }),
                ),
                plain_tail,
            );
        });
    }

    /// Authored soft breaks apply the selected continuation policy before decoding text.
    fn soft_break(&self, text: &str) -> Option<(usize, Kind)> {
        let rest = text.strip_prefix('\n')?;
        if self.options == MarkedOptions::Export
            && let Some(token) = self.line_break(text)
        {
            return Some(token);
        }
        let end = 1 + self.continuation_indent(rest);
        Some((
            end,
            Kind::Text(Text {
                authored: text[..end].to_owned(),
                decoded: "\n".to_owned(),
            }),
        ))
    }

    /// Escaped punctuation retains separate authored and display spellings.
    fn escape(text: &str) -> Option<(usize, Kind)> {
        let matched = rules::INLINE_ESCAPE.as_ref()?.find(text)?;
        Some((
            matched.end(),
            Kind::Text(Text {
                authored: text[matched.range()].to_owned(),
                decoded: capture(text, &matched, 1).to_owned(),
            }),
        ))
    }

    /// Tags remain raw only for callers that allow authored markup.
    fn tag(&self, text: &str) -> Option<(usize, Kind)> {
        if self.options == MarkedOptions::Export {
            return None;
        }
        if self.options == (MarkedOptions::Browser { escape_html: true }) {
            let html = super::html::inline(text)?;
            return Some((html.consumed, Kind::Html(html.value)));
        }
        let matched = rules::INLINE_TAG.as_ref()?.find(text)?;
        Some((matched.end(), Kind::Html(text[matched.range()].to_owned())))
    }

    /// The end of the line containing `range.start`: just past its newline, or the range end.
    fn line_end(&self, range: Range<usize>) -> usize {
        let text = &self.source.text[range.clone()];
        let end = text.find('\n').map_or(text.len(), |newline| newline + 1);
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.line_bytes += end);
        range.start + end
    }

    /// Select fallback text without crossing a typed operand boundary.
    fn text(&self, text: &str) -> usize {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.text_bytes += text.len());
        rules::inline_text(self.options)
            .and_then(|rule| rule.find(text))
            .map_or_else(
                || text.chars().next().map_or(0, char::len_utf8),
                |matched| matched.end(),
            )
    }

    /// Code normalizes newlines and trims exactly one pair of edge spaces.
    fn code(text: &str) -> Option<(usize, Kind)> {
        let matched = rules::INLINE_CODE.as_ref()?.find(text)?;
        let body = capture(text, &matched, 2).replace('\n', " ");
        let trim =
            body.starts_with(' ') && body.ends_with(' ') && body.contains(|scalar| scalar != ' ');
        let body = if trim {
            body[1..body.len() - 1].to_owned()
        } else {
            body
        };
        Some((matched.end(), Kind::Code(body)))
    }

    /// Literal markup continuations retain the browser's visible left edge.
    fn continuation_indent(&self, text: &str) -> usize {
        if self.options == (MarkedOptions::Browser { escape_html: true })
            && super::html::displayed_tag(text.trim_start_matches([' ', '\t']))
        {
            text.bytes()
                .take_while(|byte| matches!(byte, b' ' | b'\t'))
                .count()
        } else {
            0
        }
    }

    /// Breaks come from authored source, never from a decoded entity newline.
    fn line_break(&self, text: &str) -> Option<(usize, Kind)> {
        let matched = rules::inline_br(self.options)?.find(text)?;
        let end = matched.end() + self.continuation_indent(&text[matched.end()..]);
        Some((end, Kind::Break))
    }

    /// Caller-selected deletion uses the same recursive inline grammar.
    fn deletion(
        &mut self,
        range: Range<usize>,
        atoms: &[Range<usize>],
        in_link: bool,
    ) -> Option<(usize, Kind)> {
        let text = &self.source.text[range.clone()];
        if !text.starts_with('~') {
            return None;
        }
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.deletion_bytes += text.len());
        let matched = rules::inline_del(self.options)?.find(text)?;
        let body = matched.group(2)?;
        let children = self.tokens(
            range.start + body.start..range.start + body.end,
            atoms,
            in_link,
        );
        Some((matched.end(), Kind::Strike(children)))
    }

    /// Admit native operands through the token-producing recognizers.
    fn admit_operand(
        &mut self,
        range: Range<usize>,
        atoms: &[Range<usize>],
        in_link: bool,
    ) -> Option<(usize, Kind)> {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.operand_admissions += 1);
        let text = &self.source.text[range.clone()];
        match text.as_bytes().first()? {
            b'\\' => Self::escape(text),
            b'<' => self.tag(text).or_else(|| self.angle(range, in_link)),
            b'[' | b'!' => self.link(range, atoms, in_link),
            b'`' => Self::code(text),
            _ => None,
        }
    }

    /// Mask only references selected from the original document index.
    fn mask_references(&self, bytes: &mut [u8], extent: &Range<usize>) {
        if self.definitions.is_empty() {
            return;
        }
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.mask_bytes += extent.len());
        let Some(rule) = rules::inline_reflink_search(self.options) else {
            return;
        };
        for matched in rule.find_iter(&self.source.text[extent.clone()]) {
            let text =
                &self.source.text[extent.start + matched.start()..extent.start + matched.end()];
            let identity = text
                .rsplit_once('[')
                .and_then(|(_, text)| text.strip_suffix(']'))
                .unwrap_or("");
            if self
                .definitions
                .contains_key(&crate::lexer::reference_key(identity))
            {
                mask_barrier(bytes, matched.range());
            }
        }
    }

    /// Mask escaped punctuation and syntax barriers once per inline invocation.
    fn mask(&self, extent: Range<usize>, atoms: &[Range<usize>]) -> String {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.mask_bytes += extent.len());
        let mut bytes = self.source.text[extent.clone()].as_bytes().to_vec();
        self.mask_references(&mut bytes, &extent);
        if self.options == MarkedOptions::Export {
            self.mask_matches(&mut bytes, rules::inline_block_skip(self.options), true);
        }
        self.mask_matches(&mut bytes, rules::INLINE_ANY_PUNCTUATION.as_ref(), false);
        if self.options != MarkedOptions::Export {
            self.mask_matches(&mut bytes, rules::inline_block_skip(self.options), true);
        }
        for atom in atoms
            .iter()
            .filter(|range| range.start >= extent.start && range.end <= extent.end)
        {
            mask_barrier(
                &mut bytes,
                atom.start - extent.start..atom.end - extent.start,
            );
        }
        String::from_utf8(bytes).unwrap_or_else(|_| self.source.text[extent].to_owned())
    }
    /// Apply a mask phase to the preceding phase's output, not to the original input.
    fn mask_matches(&self, bytes: &mut [u8], rule: Option<&regress::Regex>, barrier: bool) {
        let Some(rule) = rule else { return };
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.mask_bytes += bytes.len());
        let ranges: Vec<_> = std::str::from_utf8(bytes)
            .ok()
            .map(|text| {
                let projected = if barrier {
                    mask_source(text, self.options).0
                } else {
                    std::borrow::Cow::Borrowed(text)
                };
                rule.find_iter(&projected)
                    .map(|matched| matched.range())
                    .collect()
            })
            .unwrap_or_default();
        for range in ranges {
            if barrier {
                mask_barrier(bytes, range);
            } else {
                bytes[range].fill(b'+');
            }
        }
    }
}

/// Whether a text node displays exactly its authored spelling.
fn plain(text: &Text) -> bool {
    #[cfg(test)]
    crate::tests::WORK.with_borrow_mut(|work| work.merge_bytes += text.authored.len());
    text.decoded == text.authored
}

/// Coalesce ordinary text without discarding authored extents or decoding twice.
///
/// `plain_tail` is the index of the plain text node this extent last created or extended,
/// so that node is not compared again; any other previous node is checked in full.
fn append_text(output: &mut Vec<Node>, node: Node, plain_tail: &mut Option<usize>) {
    let tail = output.len().checked_sub(1);
    let current_plain = matches!(&node.kind, Kind::Text(current) if plain(current));
    if current_plain
        && let Some(last) = output.last_mut()
        && let (Kind::Text(previous), Kind::Text(current)) = (&mut last.kind, &node.kind)
        && (*plain_tail == tail || plain(previous))
    {
        previous.decoded.push_str(&current.decoded);
        previous.authored.push_str(&current.authored);
        last.authored.push_str(&node.authored);
        last.range.end = node.range.end;
        *plain_tail = tail;
        return;
    }
    output.push(node);
    *plain_tail = current_plain.then(|| output.len() - 1);
}

/// Retain punctuation flanking while hiding the contents of an opaque operand.
fn mask_barrier(bytes: &mut [u8], range: Range<usize>) {
    bytes[range.clone()].fill(b'a');
    if range.len() >= 2 {
        bytes[range.start] = b'[';
        bytes[range.end - 1] = b']';
    } else if !range.is_empty() {
        bytes[range.start] = b'+';
    }
}

/// Borrowed inline syntax that a block extension cannot split, scanned once from one start of
/// a block source and reused at later starts whose own scan finds the same barriers.
pub(crate) struct Barriers {
    /// Block-source offset where the scan began.
    start: usize,
    /// Ordered, merged barriers in block-source coordinates.
    ranges: Vec<Range<usize>>,
    /// Ordered angle spans the escaped mask consumed whole, in block-source coordinates.
    spans: Vec<Range<usize>>,
}

impl Barriers {
    /// Scan the block source `text` from `start`.
    pub(crate) fn scan(text: &str, start: usize, options: MarkedOptions) -> Self {
        let text = &text[start..];
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.mask_bytes += text.len());
        let (projected, spans) = mask_source(text, options);
        let mut ranges = Vec::new();
        for rule in [
            rules::INLINE_ANY_PUNCTUATION.as_ref(),
            rules::inline_block_skip(options),
        ]
        .into_iter()
        .flatten()
        {
            ranges.extend(rule.find_iter(&projected).map(|matched| matched.range()));
        }
        ranges.sort_by_key(|range| range.start);
        let mut merged: Vec<Range<usize>> = Vec::new();
        for range in ranges {
            if let Some(previous) = merged
                .last_mut()
                .filter(|previous| previous.end >= range.start)
            {
                previous.end = previous.end.max(range.end);
            } else {
                merged.push(range);
            }
        }
        let shift = |range: Range<usize>| start + range.start..start + range.end;
        Self {
            start,
            ranges: merged.into_iter().map(shift).collect(),
            spans: spans.into_iter().map(shift).collect(),
        }
    }

    /// Whether a scan from `start` would find exactly the barriers that begin there or later:
    /// no barrier or mask span crosses it, and no backtick before it fails the code-span
    /// lookbehind, the rules' only context before a match.
    pub(crate) fn reusable_at(&self, text: &str, start: usize) -> bool {
        start == self.start
            || (start > self.start
                && !crosses(&self.ranges, start)
                && !crosses(&self.spans, start)
                && text.as_bytes()[start - 1] != b'`')
    }

    /// The barrier containing the block-source offset `position`.
    pub(crate) fn containing(&self, position: usize) -> Option<Range<usize>> {
        let index = self.ranges.partition_point(|range| range.end <= position);
        self.ranges
            .get(index)
            .filter(|range| range.contains(&position))
            .cloned()
    }
}

/// Whether one of the ordered, disjoint `ranges` starts before `position` and ends after it.
fn crosses(ranges: &[Range<usize>], position: usize) -> bool {
    let index = ranges.partition_point(|range| range.end <= position);
    ranges
        .get(index)
        .is_some_and(|range| range.start < position)
}

/// Share the caller's literal-angle mask projection between delimiters and block hints, with
/// the spans the mask consumed whole.
fn mask_source(
    text: &str,
    options: MarkedOptions,
) -> (std::borrow::Cow<'_, str>, Vec<Range<usize>>) {
    if options == (MarkedOptions::Browser { escape_html: true }) {
        let (masked, spans) = super::html::literal_mask(text);
        (std::borrow::Cow::Owned(masked), spans)
    } else {
        (std::borrow::Cow::Borrowed(text), Vec::new())
    }
}

/// Whether a token fits before the next atom: a span may contain whole atoms but never
/// end inside one.
fn fits(atoms: &[Range<usize>], end: usize, limit: usize, kind: &Kind) -> bool {
    let enclosing = atoms.get(atoms.partition_point(|range| range.end <= end));
    end <= limit
        || matches!(kind, Kind::Emphasis(_) | Kind::Strong(_) | Kind::Strike(_))
            && enclosing.is_none_or(|range| range.start >= end)
}
