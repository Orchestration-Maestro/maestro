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
        let mut cursor = extent.start;
        let masked = self.mask(extent.clone(), atoms);
        while cursor < extent.end {
            let atom = atoms.partition_point(|range| range.end <= cursor);
            if let Some(range) = atoms.get(atom).filter(|range| range.start == cursor) {
                output.push(node(self.source, range.clone(), Kind::Atom(atom)));
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
                    self.emphasis(cursor..extent.end, (&masked, extent.start), atoms, in_link)
                })
                .or_else(|| self.line_break(text))
                .or_else(|| self.deletion(cursor..extent.end, atoms, in_link));
            if let Some((size, kind)) = token.filter(|(size, kind)| {
                *size > 0
                    && (cursor + size <= limit
                        || matches!(kind, Kind::Emphasis(_) | Kind::Strong(_) | Kind::Strike(_)))
            }) {
                output.push(node(self.source, cursor..cursor + size, kind));
                cursor += size;
                continue;
            }
            let window = &text[..limit - cursor];
            // Only the first line is kept, so the recognizer never reads past its newline.
            let line = window.find('\n').map_or(window, |end| &window[..=end]);
            let size = self.text(line);
            let size = line[..size].find('\n').unwrap_or(size);
            self.push_text(cursor..cursor + size, &mut output);
            cursor += size;
        }
        output
    }

    /// Emit decoded units with their original source boundaries.
    fn push_text(&self, range: Range<usize>, output: &mut Vec<Node>) {
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
                    mask_source(text, self.options)
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

/// Coalesce ordinary text without discarding authored extents or decoding twice.
fn append_text(output: &mut Vec<Node>, node: Node) {
    if let Some(last) = output.last_mut()
        && let (Kind::Text(previous), Kind::Text(current)) = (&mut last.kind, &node.kind)
        && previous.decoded == previous.authored
        && current.decoded == current.authored
    {
        previous.decoded.push_str(&current.decoded);
        previous.authored.push_str(&current.authored);
        last.authored.push_str(&node.authored);
        last.range.end = node.range.end;
        return;
    }
    output.push(node);
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

/// Borrowed inline syntax that a block extension cannot split.
pub(crate) fn barriers(text: &str, options: MarkedOptions) -> Vec<Range<usize>> {
    #[cfg(test)]
    crate::tests::WORK.with_borrow_mut(|work| work.mask_bytes += text.len());
    let projected = mask_source(text, options);
    let text = projected.as_ref();
    let mut ranges = Vec::new();
    for rule in [
        rules::INLINE_ANY_PUNCTUATION.as_ref(),
        rules::inline_block_skip(options),
    ]
    .into_iter()
    .flatten()
    {
        ranges.extend(rule.find_iter(text).map(|matched| matched.range()));
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
    merged
}

/// Share the caller's literal-angle mask projection between delimiters and block hints.
fn mask_source(text: &str, options: MarkedOptions) -> std::borrow::Cow<'_, str> {
    if options == (MarkedOptions::Browser { escape_html: true }) {
        std::borrow::Cow::Owned(super::html::literal_mask(text))
    } else {
        std::borrow::Cow::Borrowed(text)
    }
}
