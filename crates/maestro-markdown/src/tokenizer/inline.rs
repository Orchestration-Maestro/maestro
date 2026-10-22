//! Inline precedence over original source and typed atom boundaries.
pub(crate) mod delimiters;
mod emphasis;
mod entities;
mod links;
mod tokens;
use super::{capture, node};
use crate::{
    Definition, Kind, MarkedOptions, Node, Text, inline::ScopeKey, rules, source::ScopeSource,
};
pub(crate) use entities::decode_operand;
use std::{collections::HashMap, ops::Range};

/// Link-label and raw-anchor state used by inline tokenization.
#[derive(Clone, Copy)]
pub(crate) struct LinkContext(bool);

impl LinkContext {
    /// Enter an inline scope, including an authored link or image label.
    pub(crate) fn new(label: bool) -> Self {
        Self(label)
    }

    /// Whether bare address recognition is inside a link context.
    pub(crate) fn active(self) -> bool {
        self.0
    }

    /// Follow raw anchors and completed authored labels; angle links leave context unchanged.
    pub(crate) fn track_anchor(&mut self, kind: &Kind) {
        match kind {
            Kind::Html(html) => {
                if html
                    .get(..3)
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case("<a "))
                {
                    self.0 = true;
                } else if html
                    .get(..4)
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case("</a>"))
                {
                    self.0 = false;
                }
            }
            Kind::Image(..) => self.0 = false,
            Kind::Link(_, _, _, meta) if meta.link_type != crate::LinkType::Autolink => {
                self.0 = false;
            }
            _ => {}
        }
    }
}

/// One inline invocation; original definitions and policy are never reconstructed.
pub(crate) struct Tokenizer<'a> {
    /// Source-owned delimiter observations shared by recursive calls.
    delimiters: delimiters::Delimiters,
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
    /// Parent run slots for remapped label atoms.
    pub(crate) atom_indices: Option<&'a [usize]>,
    /// Link-label and raw-anchor context shared by recursive inline spans.
    pub(crate) context: LinkContext,
}
impl<'a> Tokenizer<'a> {
    /// Bind one source view to its document state.
    pub(crate) fn new(
        source: &'a ScopeSource,
        options: MarkedOptions,
        definitions: &'a HashMap<String, Definition>,
        scopes: Option<&'a mut HashMap<ScopeKey, ScopeSource>>,
        in_link: bool,
    ) -> Self {
        Self {
            delimiters: delimiters::Delimiters::new(&source.text, options),
            source,
            options,
            definitions,
            scopes,
            context: LinkContext::new(in_link),
            retained_atoms: None,
            atom_indices: None,
        }
    }

    /// Admit inline constructs with typed atoms ahead of ordinary grammar.
    pub(crate) fn parse(
        &mut self,
        atoms: &[Range<usize>],
        admission: Option<&mut crate::inline::InlineAdmission<'_>>,
    ) -> Vec<Node> {
        self.tokens(0..self.source.text.len(), atoms, admission)
    }

    /// Retain operand identity and apply link-context transitions, never caller-opaque contents.
    fn push_atom(&mut self, range: Range<usize>, index: usize, output: &mut Vec<Node>) {
        let index = self
            .atom_indices
            .and_then(|ids| ids.get(index))
            .copied()
            .unwrap_or(index);
        if let Some(crate::InlineAtom::Resolved(original)) =
            self.retained_atoms.and_then(|atoms| atoms.get(index))
        {
            self.context.track_anchor(&original.kind);
        }
        output.push(node(self.source, range, Kind::Atom(index)));
    }

    /// Keep accepted bare addresses whole where the grammar enables URL tokens.
    fn address(&self, range: Range<usize>, email_skip: &mut usize) -> Option<(usize, String)> {
        (!self.context.active() && self.options != MarkedOptions::Documentation)
            .then(|| {
                crate::autolinks::address(
                    &self.source.text[range.clone()],
                    self.options,
                    range.start,
                    email_skip,
                )
            })
            .flatten()
    }

    /// Emit decoded units with their original source boundaries.
    fn push_text(
        &self,
        range: Range<usize>,
        output: &mut Vec<Node>,
        plain_tail: &mut Option<usize>,
    ) {
        let text = &self.source.text[range.clone()];
        let trim = text.rfind('\n').map_or(0, |newline| {
            let indent = self.continuation_indent(&self.source.text[range.start + newline + 1..]);
            if newline + 1 + indent == text.len() {
                indent
            } else {
                0
            }
        });
        entities::decode_units(text, |unit, decoded| {
            let decoded = if unit.end == text.len() {
                &decoded[..decoded.len() - trim]
            } else {
                decoded
            };
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
        admission: Option<&mut crate::inline::InlineAdmission<'_>>,
    ) -> Option<(usize, Kind)> {
        let (consumed, body) = self.delimiters.deletion(&self.source.text, &range)?;
        let children = self.tokens(body, atoms, admission);
        Some((consumed, Kind::Strike(children)))
    }

    /// Admit native operands through the token-producing recognizers.
    fn admit_operand(
        &mut self,
        range: Range<usize>,
        atoms: &[Range<usize>],
        admission: Option<&mut crate::inline::InlineAdmission<'_>>,
    ) -> Option<(usize, Kind)> {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.operand_admissions += 1);
        let text = &self.source.text[range.clone()];
        match text.as_bytes().first()? {
            b'\\' => Self::escape(text),
            b'<' => self.tag(text).or_else(|| self.angle(range)),
            b'[' | b'!' => self.link(range, atoms, admission),
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
        let first = atoms.partition_point(|atom| atom.end <= extent.start);
        let last = atoms.partition_point(|atom| atom.start < extent.end);
        for atom in atoms[first..last].iter().filter(|range| {
            #[cfg(test)]
            crate::tests::WORK.with_borrow_mut(|work| work.mask_operands += 1);
            range.start >= extent.start && range.end <= extent.end
        }) {
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
        last.authored.append(&node.authored);
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

/// Project literal angles for inline delimiter masking.
fn mask_source(text: &str, options: MarkedOptions) -> std::borrow::Cow<'_, str> {
    if options == (MarkedOptions::Browser { escape_html: true }) {
        std::borrow::Cow::Owned(super::html::literal_mask(text))
    } else {
        std::borrow::Cow::Borrowed(text)
    }
}

/// Whether a token fits before the next atom: a span may contain whole atoms but never
/// end inside one.
fn fits(atoms: &[Range<usize>], token: Range<usize>, limit: usize, kind: &Kind) -> bool {
    let end = token.end;
    let enclosing = atoms
        .get(atoms.partition_point(|range| range.end <= end))
        .filter(|range| range.start >= token.start);
    end <= limit
        || matches!(kind, Kind::Emphasis(_) | Kind::Strong(_) | Kind::Strike(_))
            && enclosing.is_none_or(|range| range.start >= end)
}
