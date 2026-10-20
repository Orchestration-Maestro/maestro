//! Scoped native inline interpretation over resolved document operands.
use crate::{Document, Kind, Node, source::ScopeSource};
use std::{collections::HashMap, ops::Range};

/// Native scope identity: start, end and whether this is an authored link label.
pub(crate) type ScopeKey = (usize, usize, bool);

/// A document's inline scopes, indexed once after mapped link selection.
pub struct InlineContext<'a> {
    /// Container and label scopes keyed by native extent and label classification.
    scopes: HashMap<(usize, usize, bool), InlineScope<'a>>,
}
/// Original inline source with resolved, locally mapped operands.
pub struct InlineScope<'a> {
    /// Authored container bytes and their native provenance.
    source: &'a ScopeSource,
    /// An authored label retains its raw source for caller admission.
    label: bool,
    /// Actual semantic child extent within the retained authored scope.
    content: Range<usize>,
    /// Immutable original-document grammar.
    options: crate::MarkedOptions,
    /// First-wins definitions retained by the original document.
    definitions: &'a HashMap<String, crate::Definition>,
    /// Native opaque operands, in authored order.
    pub operands: Vec<(Range<usize>, &'a Node)>,
    /// Code, image, HTML and autolink barriers, including those inside authored labels.
    pub opaque: Vec<Range<usize>>,
    /// Strong, strike and emphasis extents with their content extents, in authored order.
    pub spans: Vec<(Range<usize>, Range<usize>)>,
}
/// A locally interpreted inline run and its retained atom identities.
pub struct InlineRun<'a> {
    /// Native inline grammar with generated spans lowered to atoms.
    pub nodes: Vec<Node>,
    /// Operands indexed by each generated atom.
    pub atoms: Vec<InlineAtom<'a>>,
}
/// An opaque caller span or an already resolved native operand.
pub enum InlineAtom<'a> {
    /// Index in the caller's admitted span list.
    Opaque(usize),
    /// Original native operand; its identity is never parsed again.
    Resolved(&'a Node),
}
impl<'a> InlineContext<'a> {
    /// Index inline scopes without cloning the document or its operands.
    #[must_use]
    pub fn new(document: &'a Document) -> Self {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.indexes += 1);
        let mut context = Self {
            scopes: HashMap::new(),
        };
        context.visit(&document.nodes, document);
        context
    }
    /// Find a container or authored link-label scope by its original extent.
    #[must_use]
    pub fn scope(&self, range: &Range<usize>, label: bool) -> Option<&InlineScope<'a>> {
        self.scopes.get(&(range.start, range.end, label))
    }
    /// Register one scope and its immediate resolved operands.
    fn insert(
        &mut self,
        range: &Range<usize>,
        label: bool,
        children: &'a [Node],
        document: &'a Document,
    ) {
        if let Some(source) = document.scopes.get(&(range.start, range.end, label)) {
            let mut scope = InlineScope {
                source,
                label,
                content: children
                    .first()
                    .zip(children.last())
                    .and_then(|(first, last)| source.local(&(first.range.start..last.range.end)))
                    .unwrap_or(0..0),
                options: document.options,
                definitions: &document.definitions,
                operands: Vec::new(),
                opaque: Vec::new(),
                spans: Vec::new(),
            };
            collect(children, &mut scope, false);
            self.scopes.insert((range.start, range.end, label), scope);
        }
    }
    /// Index each table cell in its own inline context.
    fn cells(&mut self, table: &'a crate::Table, document: &'a Document) {
        for cell in table.header.iter().chain(table.rows.iter().flatten()) {
            self.insert(&cell.range, false, &cell.children, document);
            self.visit(&cell.children, document);
        }
    }
    /// Traverse native containers once, including table cells and link labels.
    fn visit(&mut self, nodes: &'a [Node], document: &'a Document) {
        for node in nodes {
            #[cfg(test)]
            crate::tests::WORK.with_borrow_mut(|work| work.visits += 1);
            match &node.kind {
                Kind::Paragraph(c) | Kind::Inline(c) | Kind::Heading(_, c, _) => {
                    self.insert(&node.range, false, c, document);
                    self.visit(c, document);
                }
                Kind::Link(c, ..) => {
                    self.insert(&node.range, true, c, document);
                    self.visit(c, document);
                }
                Kind::Item(c)
                | Kind::List(_, c)
                | Kind::Quote(c, _)
                | Kind::Strong(c)
                | Kind::Strike(c)
                | Kind::Emphasis(c) => self.visit(c, document),
                Kind::Table(table) => self.cells(table, document),
                _ => {}
            }
        }
    }
}
/// Index resolved operands and nested opaque barriers in the same mapped source.
fn collect<'a>(nodes: &'a [Node], scope: &mut InlineScope<'a>, label: bool) {
    for node in nodes {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.visits += 1);
        match &node.kind {
            Kind::Strong(c) | Kind::Strike(c) | Kind::Emphasis(c) => {
                let content = c.first().zip(c.last());
                let content = content.map(|(first, last)| first.range.start..last.range.end);
                if let Some((span, content)) = scope
                    .source
                    .local(&node.range)
                    .zip(content.and_then(|content| scope.source.local(&content)))
                {
                    scope.spans.push((span, content));
                }
                collect(c, scope, label);
            }
            Kind::Link(c, _, _, meta) => {
                operand(node, scope, label);
                if matches!(
                    meta.link_type,
                    crate::LinkType::Autolink | crate::LinkType::Email
                ) {
                    scope.opaque.extend(scope.source.local(&node.range));
                } else {
                    collect(c, scope, true);
                }
            }
            Kind::Code(_) | Kind::Html(_) | Kind::Image(..) => {
                operand(node, scope, label);
                scope.opaque.extend(scope.source.local(&node.range));
            }
            _ => {}
        }
    }
}
/// Retain an immediate resolved operand, not a duplicate of an enclosing anchor's child.
fn operand<'a>(node: &'a Node, scope: &mut InlineScope<'a>, label: bool) {
    if !label && let Some(range) = scope.source.local(&node.range) {
        scope.operands.push((range, node));
    }
}
impl<'a> InlineScope<'a> {
    /// Original source used for admission, including native container framing.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.source.text
    }

    /// Semantic content selected by the original parser, excluding container padding.
    #[must_use]
    pub fn content_range(&self) -> Range<usize> {
        self.content.clone()
    }

    /// Interpret a subrange with caller-admitted spans ahead of ordinary inline grammar.
    /// Resolved operands survive unless an earlier admitted span consumes their syntax.
    /// The range and sorted disjoint spans must name UTF-8 boundaries in `text()`;
    /// each span must be contained in the range.
    #[must_use]
    pub fn interpret(&self, range: Range<usize>, opaque: &[Range<usize>]) -> InlineRun<'a> {
        let mut selected: Vec<_> = opaque
            .iter()
            .enumerate()
            .map(|(id, span)| (span.clone(), InlineAtom::Opaque(id)))
            .collect();
        let mut next = 0;
        for (span, node) in self.within(&range) {
            #[cfg(test)]
            crate::tests::WORK.with_borrow_mut(|work| work.operand_examinations += 1);
            while opaque.get(next).is_some_and(|atom| atom.end <= span.start) {
                next += 1;
            }
            let overlaps = opaque.get(next).is_some_and(|atom| atom.start < span.end);
            if range.start <= span.start && span.end <= range.end && !overlaps {
                selected.push((span.clone(), InlineAtom::Resolved(node)));
            }
        }
        selected.sort_by_key(|(span, _)| span.start);
        let source = self.source.select(std::iter::once(range));
        let source = if self.label && self.options == crate::MarkedOptions::Export {
            source.brackets()
        } else {
            source
        };
        let mut slots = Vec::new();
        let mut atoms = Vec::new();
        for (span, atom) in selected {
            if let Some(local) = source.local(&self.source.extent(span)) {
                slots.push(local);
                atoms.push(atom);
            }
        }
        let mut tokenizer = crate::tokenizer::inline::Tokenizer::new(
            &source,
            self.options,
            self.definitions,
            None,
            self.label,
        );
        tokenizer.retained_atoms = Some(&atoms);
        let children = tokenizer.parse(&slots);
        InlineRun {
            nodes: vec![crate::tokenizer::node(
                &source,
                0..source.text.len(),
                Kind::Paragraph(children),
            )],
            atoms,
        }
    }

    /// Select source-ordered operands by start offset before examining overlaps.
    fn within(&self, range: &Range<usize>) -> &[(Range<usize>, &'a Node)] {
        let boundary = |offset| {
            self.operands.partition_point(|(span, _)| {
                #[cfg(test)]
                crate::tests::WORK.with_borrow_mut(|work| work.operand_examinations += 1);
                span.start < offset
            })
        };
        &self.operands[boundary(range.start)..boundary(range.end)]
    }
}
