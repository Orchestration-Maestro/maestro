//! Ordered matching and scalar-safe cursor progress.
use super::{
    Tables,
    types::{Rule, RuleKind},
};

/// A match into the original text, including unmodified captures.
pub(super) struct Hit<'a> {
    /// First matching byte.
    pub index: usize,
    /// Byte after the match.
    pub end: usize,
    /// Original matched text.
    pub text: &'a str,
    /// Captured text in source group order.
    pub captures: Vec<Option<&'a str>>,
    /// Winning rule's action.
    pub kind: RuleKind,
    /// Winning rule's original order.
    pub rule_index: usize,
}

/// Earliest match wins; original rule order breaks equal-position ties.
fn search<'a>(tables: &Tables, rules: &[Rule], text: &'a str, cursor: &Cursor) -> Option<Hit<'a>> {
    rules
        .iter()
        .enumerate()
        .skip(cursor.resume)
        .filter_map(|(index, rule)| {
            tables.patterns[rule.pattern.0]
                .find_from(text, cursor.position)
                .next()
                .map(|found| Hit {
                    index: found.range.start,
                    end: found.range.end,
                    text: &text[found.range],
                    captures: found
                        .captures
                        .iter()
                        .map(|r| r.as_ref().map(|r| &text[r.clone()]))
                        .collect(),
                    kind: rule.action,
                    rule_index: index,
                })
        })
        .min_by_key(|hit| (hit.index, hit.rule_index))
}

/// Position and same-position ignored-begin resumption.
#[derive(Default)]
pub(super) struct Cursor {
    /// Current byte position in the original text.
    pub position: usize,
    /// First eligible rule at this position.
    pub resume: usize,
    /// Previous transition for zero-width begin/end progress.
    pub last: Option<(RuleKind, usize)>,
}

impl Cursor {
    /// Resume remaining rules only at the ignored match's position.
    pub fn find<'a>(&self, tables: &Tables, rules: &[Rule], text: &'a str) -> Option<Hit<'a>> {
        if self.position > text.len() {
            return None;
        }
        let hit = search(tables, rules, text, self);
        if self.resume == 0 || hit.as_ref().is_some_and(|h| h.index == self.position) {
            return hit;
        }
        (self.position < text.len())
            .then(|| {
                search(
                    tables,
                    rules,
                    text,
                    &Self {
                        position: self.position + scalar_len(text, self.position),
                        ..Self::default()
                    },
                )
            })
            .flatten()
    }

    /// Consume one intact scalar, including a nonmatching ignored boundary.
    pub fn advance(&mut self, text: &str, position: usize, buffer: &mut String) {
        self.skip(text, position);
        buffer.push_str(text.get(position..self.position).unwrap_or_default());
    }
    /// Skip one scalar without contributing its text to a scoring buffer.
    pub fn skip(&mut self, text: &str, position: usize) {
        self.position = position + scalar_len(text, position);
    }
}

/// UTF-8 progress; one byte past EOF terminates empty final matches.
fn scalar_len(text: &str, position: usize) -> usize {
    text.get(position..)
        .and_then(|tail| tail.chars().next())
        .map_or(1, char::len_utf8)
}
