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
    /// Native opaque operands, in authored order.
    pub operands: Vec<(Range<usize>, &'a Node)>,
    /// Code, image, HTML and autolink barriers, including those inside authored labels.
    pub opaque: Vec<Range<usize>>,
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
                operands: Vec::new(),
                opaque: Vec::new(),
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
            Kind::Strong(c) | Kind::Strike(c) | Kind::Emphasis(c) => collect(c, scope, label),
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
        for (span, node) in &self.operands {
            while opaque.get(next).is_some_and(|atom| atom.end <= span.start) {
                next += 1;
            }
            let overlaps = opaque.get(next).is_some_and(|atom| atom.start < span.end);
            if range.start <= span.start && span.end <= range.end && !overlaps {
                selected.push((span.clone(), InlineAtom::Resolved(node)));
            }
        }
        selected.sort_by_key(|(span, _)| span.start);
        let mut generated = String::new();
        let mut slots = Vec::new();
        let mut atoms = Vec::new();
        let mut end = range.start;
        for (span, atom) in selected {
            generated.push_str(&self.text()[end..span.start]);
            let start = generated.len();
            generated.push_str("<m></m>");
            slots.push(start..generated.len());
            atoms.push(atom);
            end = span.end;
        }
        generated.push_str(&self.text()[end..range.end]);
        let mut document = crate::parse::native(generated, None);
        lower(&mut document.nodes, &slots);
        InlineRun {
            nodes: document.nodes,
            atoms,
        }
    }
}
/// Decode only generated spans, never authored marker-looking HTML.
fn lower(nodes: &mut Vec<Node>, slots: &[Range<usize>]) {
    nodes.retain_mut(|node| {
        match &mut node.kind {
            Kind::Paragraph(c)
            | Kind::Inline(c)
            | Kind::Heading(_, c, _)
            | Kind::List(_, c)
            | Kind::Item(c)
            | Kind::Quote(c, _)
            | Kind::Strong(c)
            | Kind::Strike(c)
            | Kind::Emphasis(c) => lower(c, slots),
            Kind::Html(_) => return lower_atom(node, slots),
            _ => {}
        }
        true
    });
}

/// Replace a generated opening marker and remove its matching closing marker.
fn lower_atom(node: &mut Node, slots: &[Range<usize>]) -> bool {
    let index = slots.partition_point(|span| span.end <= node.range.start);
    let Some(span) = slots
        .get(index)
        .filter(|span| span.contains(&node.range.start))
    else {
        return true;
    };
    if span.start != node.range.start {
        return false;
    }
    node.kind = Kind::Atom(index);
    true
}
