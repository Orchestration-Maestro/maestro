//! Flat group ownership and authored delimiter scanning.

use super::{Ast, Token};
use crate::minimatch::brace_expressions::class_boundary;
use std::ops::Range;

/// Cardinality of an authored group.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Operator {
    /// One alternative.
    Choice,
    /// Zero or one alternative.
    Optional,
    /// Zero or more alternatives.
    Repeat,
    /// One or more alternatives.
    Required,
    /// Excludes alternatives with their continuation.
    Negative,
}

impl Operator {
    /// Recognizes an unescaped operator.
    fn from_scalar(c: char) -> Option<Self> {
        match c {
            '@' => Some(Self::Choice),
            '?' => Some(Self::Optional),
            '*' => Some(Self::Repeat),
            '+' => Some(Self::Required),
            '!' => Some(Self::Negative),
            _ => None,
        }
    }
}

/// An authored group with structural enclosing links.
pub(super) struct Group {
    /// Operator selecting endpoint cardinality.
    pub(super) operator: Operator,
    /// Alternative sequence IDs.
    pub(super) alternatives: Vec<usize>,
    /// Enclosing group, if any.
    pub(super) parent: Option<usize>,
    /// Containing sequence ID.
    pub(super) outer: usize,
    /// Position within the containing sequence.
    pub(super) position: usize,
    /// Authored bytes for literal fallback.
    pub(super) span: Range<usize>,
}

/// Flat syntax with one retained authored component.
pub(in crate::minimatch) struct Groups {
    /// Original bytes, used only for fallback.
    pub(super) source: String,
    /// Groups in opening-delimiter order.
    pub(super) nodes: Vec<Group>,
    /// Owned token sequences, with the root at zero.
    pub(super) sequences: Vec<Vec<Token>>,
}

/// Delimiter shielding within an authored bracket region.
#[derive(Default)]
struct Shield {
    /// Escape awaiting its next scalar.
    escaped: bool,
    /// Exclusive byte boundary of the current class, or component end if unclosed.
    bracket_end: usize,
}

impl Shield {
    /// Consumes shielded scalars without interpreting group delimiters.
    fn consumes(&mut self, pattern: &str, c: char, at: usize) -> bool {
        if at < self.bracket_end {
            return true;
        }
        if self.escaped || c == '\\' {
            self.escaped = !self.escaped;
            return true;
        }
        if c == '[' {
            self.bracket_end = at + class_boundary(&pattern[at..]);
            return true;
        }
        false
    }
}

impl Groups {
    /// Parses groups iteratively, retaining malformed outer text as basic syntax.
    pub(super) fn parse(pattern: &str) -> Option<Self> {
        let mut model = None;
        let mut stack = Vec::new();
        let mut sequence = 0;
        let mut begin = 0;
        let mut shield = Shield::default();
        let mut chars = pattern.char_indices().peekable();
        while let Some((at, c)) = chars.next() {
            if shield.consumes(pattern, c, at) {
                continue;
            }
            if let Some(operator) = Operator::from_scalar(c)
                && chars.peek().is_some_and(|(_, next)| *next == '(')
            {
                let model = model.get_or_insert_with(|| Self {
                    source: pattern.into(),
                    nodes: Vec::new(),
                    sequences: vec![Vec::new()],
                });
                model.append(sequence, &pattern[begin..at]);
                let id = model.open(operator, sequence, stack.last().copied(), at);
                stack.push(id);
                sequence = model.new_sequence();
                chars.next();
                begin = at + 2;
            } else if matches!(c, '|' | ')')
                && let Some(&id) = stack.last()
            {
                let model = model.as_mut()?;
                model.append(sequence, &pattern[begin..at]);
                model.nodes[id].alternatives.push(sequence);
                sequence = model.finish_delimiter(id, c, at, &mut stack);
                begin = at + 1;
            }
        }
        let mut model = model?;
        model.append(sequence, &pattern[begin..]);
        if let Some(&id) = stack.first() {
            let group = &model.nodes[id];
            let (outer, position, begin) = (group.outer, group.position, group.span.start);
            model.sequences[outer].truncate(position);
            model.append(outer, &pattern[begin..]);
            model.nodes.truncate(id);
        }
        (!model.nodes.is_empty()).then_some(model)
    }

    /// Records a group at its enclosing syntax position.
    fn open(
        &mut self,
        operator: Operator,
        outer: usize,
        parent: Option<usize>,
        begin: usize,
    ) -> usize {
        let id = self.nodes.len();
        self.nodes.push(Group {
            operator,
            alternatives: Vec::new(),
            parent,
            outer,
            position: self.sequences[outer].len(),
            span: begin..self.source.len(),
        });
        self.sequences[outer].push(Token::Group(id));
        id
    }

    /// Opens another owned sequence.
    fn new_sequence(&mut self) -> usize {
        let id = self.sequences.len();
        self.sequences.push(Vec::new());
        id
    }

    /// Ends a branch or returns to its enclosing sequence.
    fn finish_delimiter(&mut self, id: usize, c: char, at: usize, stack: &mut Vec<usize>) -> usize {
        if c == '|' {
            return self.new_sequence();
        }
        stack.pop();
        self.nodes[id].span.end = at + 1;
        self.nodes[id].outer
    }

    /// Delegates literal decoding and scalar syntax to the basic parser.
    fn append(&mut self, sequence: usize, text: &str) {
        let tokens = match Ast::basic(text) {
            Ast::Literal(text) => text.chars().map(Token::Literal).collect(),
            Ast::Pattern(tokens) | Ast::Ignore(tokens) => tokens,
            Ast::Groups(_) => return,
        };
        self.sequences[sequence].extend(tokens);
    }
}
