//! Cached endpoint evaluation over flat syntax and scalar positions.

use super::parse::{Groups, Operator};
use super::{Token, advance};
use std::collections::BTreeSet;

/// Syntax boundary and repetition-dot mode.
#[derive(Clone, Copy)]
struct Context {
    /// Sequence begins at a syntactic start.
    start: bool,
    /// Sequence ends at the component boundary.
    end: bool,
    /// Later repetitions permit a dot at their start.
    dot: bool,
}

impl Context {
    /// Decodes a table's boundary mode.
    fn from_index(index: usize) -> Self {
        Self {
            start: index & 1 != 0,
            end: index & 2 != 0,
            dot: index & 4 != 0,
        }
    }
    /// Selects the cached boundary table.
    fn index(self) -> usize {
        usize::from(self.start) | (usize::from(self.end) << 1) | (usize::from(self.dot) << 2)
    }
}

/// Deduplicated scalar endpoints.
type Endpoints = BTreeSet<usize>;
/// Endpoint sets indexed by context and starting scalar.
type Table = Vec<Vec<Endpoints>>;

/// Per-input group evaluation state.
struct Evaluation<'a> {
    /// Borrowed flat syntax.
    model: &'a Groups,
    /// Unicode scalar input.
    input: Vec<char>,
    /// Whole component is a traversal name.
    traversal: bool,
    /// Child and right-hand endpoint dependencies.
    tables: Vec<Table>,
}

/// Matches a component after resolving child and right-hand dependencies.
pub(super) fn matches(model: &Groups, text: &str) -> bool {
    let mut evaluation = Evaluation {
        model,
        input: text.chars().collect(),
        traversal: matches!(text, "." | ".."),
        tables: (0..model.nodes.len()).map(|_| Vec::new()).collect(),
    };
    for id in (0..model.nodes.len()).rev() {
        evaluation.tables[id] = (0..8)
            .map(|mode| evaluation.group_table(id, Context::from_index(mode)))
            .collect();
    }
    let root: Vec<_> = model.sequences[0].iter().collect();
    evaluation
        .sequence(
            &root,
            0,
            Context {
                start: true,
                end: true,
                dot: false,
            },
        )
        .contains(&evaluation.input.len())
}

impl Evaluation<'_> {
    /// Applies the branch's basic-token dot and traversal guards.
    fn allowed(&self, tokens: &[&Token], from: usize, context: Context) -> bool {
        if !context.start || tokens.is_empty() || matches!(tokens[0], Token::Group(_)) {
            return true;
        }
        let literal_dots = tokens.iter().all(|t| matches!(t, Token::Literal('.')));
        if literal_dots && tokens.len() <= 2 {
            return true;
        }
        let magic = |t: Option<&&Token>| {
            t.is_some_and(|t| matches!(t, Token::Star | Token::Question | Token::Class(_)))
        };
        let dot = |index: usize| {
            tokens
                .get(index)
                .is_some_and(|t| matches!(t, Token::Literal('.')))
        };
        let first_magic = magic(tokens.first());
        if !context.dot && first_magic && self.input.get(from) == Some(&'.') {
            return false;
        }
        let guarded = (context.dot && first_magic)
            || (dot(0) && (magic(tokens.get(1)) || (dot(1) && (magic(tokens.get(2)) || dot(2)))));
        !(guarded && from == 0 && self.traversal)
    }

    /// Walks deduplicated syntax/scalar states through borrowed token references.
    fn sequence(&self, tokens: &[&Token], from: usize, context: Context) -> Endpoints {
        if !self.allowed(tokens, from, context) {
            return Endpoints::new();
        }
        let noempty = context.start
            && context.end
            && !tokens.is_empty()
            && tokens.iter().all(|t| matches!(t, Token::Star));
        let mut at_start = context.start;
        let starts: Vec<_> = tokens.iter().map(|token| {
            let start = at_start;
            at_start = at_start && matches!(token, Token::Group(id) if self.model.nodes[*id].operator == Operator::Negative);
            start
        }).collect();
        let mut pending = vec![(0, from)];
        let mut seen = BTreeSet::new();
        let mut result = Endpoints::new();
        while let Some((at, position)) = pending.pop() {
            if !seen.insert((at, position)) {
                continue;
            }
            let Some(token) = tokens.get(at) else {
                result.extend((!noempty || position > from).then_some(position));
                continue;
            };
            if let Token::Group(id) = token {
                let mode = Context {
                    start: starts[at],
                    end: context.end && at + 1 == tokens.len(),
                    ..context
                };
                pending.extend(
                    self.tables[*id][mode.index()][position]
                        .iter()
                        .map(|end| (at + 1, *end)),
                );
                continue;
            }
            pending.extend(matches!(token, Token::Star).then_some((at + 1, position)));
            if let Some(&scalar) = self.input.get(position)
                && let Some(next) = advance(std::slice::from_ref(*token), 0, scalar)
            {
                pending.push((at + next, position + 1));
            }
        }
        result
    }

    /// Caches each alternative union once per start before cardinality selection.
    fn group_table(&self, id: usize, context: Context) -> Vec<Endpoints> {
        let group = &self.model.nodes[id];
        let alternatives: Vec<Vec<_>> = group
            .alternatives
            .iter()
            .map(|&sequence| &self.model.sequences[sequence])
            .filter(|tokens| !(context.start && context.end && tokens.is_empty()))
            .map(|tokens| tokens.iter().collect())
            .collect();
        if group.operator == Operator::Negative {
            return (0..=self.input.len())
                .map(|from| self.negative(id, from, context))
                .collect();
        }
        if context.start && context.end && alternatives.is_empty() {
            let literal: Vec<_> = self.model.source[group.span.clone()].chars().collect();
            return (0..=self.input.len())
                .map(|from| self.fallback(&literal, from))
                .collect();
        }
        let first = self.once(&alternatives, context);
        let later = if matches!(group.operator, Operator::Repeat | Operator::Required) {
            self.once(
                &alternatives,
                Context {
                    dot: true,
                    ..context
                },
            )
        } else {
            Vec::new()
        };
        first
            .into_iter()
            .enumerate()
            .map(|(from, hits)| Self::cardinality(group.operator, from, hits, &later))
            .collect()
    }

    /// Matches the authored empty-form fallback literally.
    fn fallback(&self, literal: &[char], from: usize) -> Endpoints {
        if self.input[from..].starts_with(literal) {
            Endpoints::from([from + literal.len()])
        } else {
            Endpoints::new()
        }
    }

    /// Unions alternatives for each starting scalar once in this boundary mode.
    fn once(&self, alternatives: &[Vec<&Token>], context: Context) -> Vec<Endpoints> {
        (0..=self.input.len())
            .map(|from| {
                alternatives
                    .iter()
                    .flat_map(|tokens| self.sequence(tokens, from, context))
                    .collect()
            })
            .collect()
    }

    /// Selects endpoint cardinality without duplicating accepted sets.
    fn cardinality(
        operator: Operator,
        from: usize,
        first: Endpoints,
        later: &[Endpoints],
    ) -> Endpoints {
        let mut hits = if matches!(operator, Operator::Repeat | Operator::Required) {
            Self::closure(&first, later)
        } else {
            first
        };
        if matches!(operator, Operator::Optional | Operator::Repeat) {
            hits.insert(from);
        }
        hits
    }

    /// Expands each later-iteration scalar state once, independent of acceptance.
    fn closure(first: &Endpoints, later: &[Endpoints]) -> Endpoints {
        let mut pending: Vec<_> = first.iter().copied().collect();
        let mut expanded = Endpoints::new();
        while let Some(position) = pending.pop() {
            if expanded.insert(position) {
                pending.extend(later[position].iter().filter(|p| !expanded.contains(p)));
            }
        }
        expanded
    }

    /// Tests exclusions with borrowed structural tails through component end.
    fn negative(&self, id: usize, from: usize, context: Context) -> Endpoints {
        let group = &self.model.nodes[id];
        let empty =
            group.alternatives.len() == 1 && self.model.sequences[group.alternatives[0]].is_empty();
        let barred = !empty
            && group.alternatives.iter().any(|&alternative| {
                let mut tokens: Vec<_> = self.model.sequences[alternative].iter().collect();
                let mut enclosing = Some(id);
                while let Some(id) = enclosing {
                    let current = &self.model.nodes[id];
                    tokens
                        .extend(self.model.sequences[current.outer][current.position + 1..].iter());
                    enclosing = current.parent;
                }
                self.sequence(
                    &tokens,
                    from,
                    Context {
                        end: true,
                        ..context
                    },
                )
                .contains(&self.input.len())
            });
        if barred || (context.start && !context.dot && self.input.get(from) == Some(&'.')) {
            return Endpoints::new();
        }
        (from + usize::from(empty)..=self.input.len()).collect()
    }
}
