//! Scoped native inline interpretation over resolved document operands.
use crate::{Document, Kind, Node, source::ScopeSource, tokenizer::inline::LinkContext};
use std::{collections::HashMap, ops::Range};

/// A requested caller token with its caller-owned identity.
pub struct InlineToken {
    /// Requested UTF-8 prefix length; zero or invalid boundaries are ignored.
    pub length: usize,
    /// Caller-owned payload identity.
    pub index: usize,
}
/// Caller-owned inline grammar tried before built-in grammar at each reached cursor.
pub trait InlineTokenizer {
    /// Locate a possible start in fallback text, as a UTF-8 byte offset.
    fn start(&self, source: &str) -> Option<usize>;
    /// Select a nonempty input prefix and its caller-owned payload identity.
    fn tokenize(&mut self, source: &str) -> Option<InlineToken>;
}
/// Per-interpretation admission state shared by recursive child windows.
pub(crate) struct InlineAdmission<'a> {
    /// Caller implementation.
    pub tokenizer: &'a mut dyn InlineTokenizer,
    /// Opaque native source intervals, including barriers in authored labels.
    pub barriers: Vec<Range<usize>>,
    /// Caller identities admitted into generated slots.
    pub ids: Vec<usize>,
    /// Fixed retained slots preceding generated slots.
    pub offset: usize,
}
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
    /// Link-context snapshots after each resolved operand, indexed by its end offset.
    contexts: Vec<(usize, LinkContext)>,
    /// Code, image and autolink barriers, including those inside authored labels.
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
                contexts: Vec::new(),
                opaque: Vec::new(),
                spans: Vec::new(),
            };
            collect(children, &mut scope, false);
            let mut context = LinkContext::new(label);
            scope.contexts = scope
                .operands
                .iter()
                .map(|(span, node)| {
                    context.track_anchor(&node.kind);
                    (span.end, context)
                })
                .collect();
            self.scopes.insert((range.start, range.end, label), scope);
        }
    }
    /// Traverse native containers once, including table cells and link labels.
    fn visit(&mut self, nodes: &'a [Node], document: &'a Document) {
        let mut pending = vec![Step::Nodes(nodes.iter())];
        while let Some(step) = pending.pop() {
            self.traverse(step, document, &mut pending);
        }
    }
    /// Admit one pending traversal step.
    fn traverse(&mut self, step: Step<'a>, document: &'a Document, pending: &mut Vec<Step<'a>>) {
        match step {
            Step::Cell(cell) => {
                self.insert(&cell.range, false, &cell.children, document);
                pending.push(Step::Nodes(cell.children.iter()));
            }
            Step::Nodes(mut nodes) => {
                let Some(node) = nodes.next() else {
                    return;
                };
                pending.push(Step::Nodes(nodes));
                self.admit(node, document, pending);
            }
        }
    }
    /// Admit one visited node's own scope and queue the children behind its siblings.
    fn admit(&mut self, node: &'a Node, document: &'a Document, pending: &mut Vec<Step<'a>>) {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.visits += 1);
        match &node.kind {
            Kind::Paragraph(c) | Kind::Inline(c) | Kind::Heading(_, c, _) => {
                self.insert(&node.range, false, c, document);
                pending.push(Step::Nodes(c.iter()));
            }
            Kind::Link(c, ..) => {
                self.insert(&node.range, true, c, document);
                pending.push(Step::Nodes(c.iter()));
            }
            Kind::Item(c)
            | Kind::List(_, c)
            | Kind::Quote(c, _)
            | Kind::Strong(c)
            | Kind::Strike(c)
            | Kind::Emphasis(c) => pending.push(Step::Nodes(c.iter())),
            Kind::Table(table) => pending.extend(
                table
                    .header
                    .iter()
                    .chain(table.rows.iter().flatten())
                    .rev()
                    .map(Step::Cell),
            ),
            _ => {}
        }
    }
}
/// One pending step of a container traversal.
enum Step<'a> {
    /// Siblings still to visit, in authored order.
    Nodes(std::slice::Iter<'a, Node>),
    /// A table cell whose children follow its own scope.
    Cell(&'a crate::Cell),
}
/// Index resolved operands and nested opaque barriers in the same mapped source.
fn collect<'a>(nodes: &'a [Node], scope: &mut InlineScope<'a>, label: bool) {
    let mut pending = vec![(nodes.iter(), label)];
    while let Some((mut nodes, label)) = pending.pop() {
        let Some(node) = nodes.next() else { continue };
        pending.push((nodes, label));
        collect_node(node, label, scope, &mut pending);
    }
}
/// Retain one visited node's operands and queue the children behind its siblings.
fn collect_node<'a>(
    node: &'a Node,
    label: bool,
    scope: &mut InlineScope<'a>,
    pending: &mut Vec<(std::slice::Iter<'a, Node>, bool)>,
) {
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
            pending.push((c.iter(), label));
        }
        Kind::Link(c, _, _, meta) => {
            operand(node, scope, label);
            if meta.link_type == crate::LinkType::Autolink {
                scope.opaque.extend(scope.source.local(&node.range));
            } else {
                pending.push((c.iter(), true));
            }
        }
        Kind::Html(_) => operand(node, scope, label),
        Kind::Code(_) | Kind::Image(..) => {
            operand(node, scope, label);
            scope.opaque.extend(scope.source.local(&node.range));
        }
        _ => {}
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
        self.interpret_inner(range, opaque, None)
    }

    /// Interpret a selected window with cursor-time caller admission.
    /// Caller tokens keep their contents visible to delimiter matching, unlike static atoms.
    /// Code, image and autolink barriers cannot be consumed by caller tokens.
    /// An earlier token may consume an HTML tag; retained tags are never entered.
    /// The range must name valid UTF-8 boundaries in `text()`.
    #[must_use]
    pub fn interpret_with_tokenizer(
        &self,
        range: Range<usize>,
        tokenizer: &mut dyn InlineTokenizer,
    ) -> InlineRun<'a> {
        let first = self.opaque.partition_point(|r| r.end <= range.start);
        let last = self.opaque.partition_point(|r| r.start < range.end);
        let mut admission = InlineAdmission {
            tokenizer,
            barriers: self.opaque[first..last]
                .iter()
                .map(|r| self.source.extent(r.clone()))
                .collect(),
            ids: Vec::new(),
            offset: 0,
        };
        let mut run = self.interpret_inner(range, &[], Some(&mut admission));
        run.atoms
            .extend(admission.ids.into_iter().map(InlineAtom::Opaque));
        run
    }

    /// Share mapped source and retained operand selection between both admission policies.
    fn interpret_inner(
        &self,
        range: Range<usize>,
        opaque: &[Range<usize>],
        mut admission: Option<&mut InlineAdmission<'_>>,
    ) -> InlineRun<'a> {
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
        let boundary = self
            .contexts
            .partition_point(|(end, _)| *end <= range.start);
        let in_link = self.contexts[..boundary]
            .last()
            .map_or(self.label, |(_, context)| context.active());
        let source = self.source.select(std::iter::once(range));
        let source = if self.label
            && (self.options == crate::MarkedOptions::Export || admission.is_some())
        {
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
            in_link,
        );
        tokenizer.retained_atoms = Some(&atoms);
        if let Some(admission) = admission.as_deref_mut() {
            admission.offset = atoms.len();
        }
        let children = tokenizer.parse(&slots, admission);
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
