//! Ordered matching and scalar-safe cursor progress.
use super::{
    Expression, Failure, Tables, published,
    types::{Mode, ModeId, Rule, RuleKind},
};
use regress::Regex;
use std::{fmt::Write, sync::Arc};

#[cfg(test)]
mod tests;

#[cfg(test)]
thread_local! {
    /// Input bytes traversed by native searches on this test thread.
    static SEARCH_BYTES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

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

/// A rule's wrapper capture and original capture range in the alternation.
struct MatchGroup {
    /// Zero-based wrapper index in the native capture array.
    offset: usize,
    /// Number of captures inside the original rule.
    count: usize,
    /// Action associated with this alternative.
    kind: RuleKind,
    /// Original rule order, including alternatives omitted during resumption.
    rule_index: usize,
}

/// One earliest-match search across an ordered set of terminators.
pub(super) struct MultiRegex {
    /// Compiled alternation, absent when there are no terminators.
    regex: Option<Regex>,
    /// Wrapper captures in original rule order.
    groups: Vec<MatchGroup>,
}

impl MultiRegex {
    /// Wrap alternatives and relocate their numeric backreferences.
    fn compile(
        patterns: &[Expression],
        rules: &[Rule],
        start: usize,
        rewriter: &Regex,
    ) -> Result<Self, Failure> {
        let mut source = String::new();
        let mut groups = Vec::new();
        let mut offset = 0;
        for (rule_index, rule) in rules.iter().enumerate().skip(start) {
            let pattern = &patterns[rule.pattern.0];
            if !source.is_empty() {
                source.push('|');
            }
            source.push('(');
            rewrite_backreferences(&mut source, &pattern.pattern, offset + 1, rewriter)?;
            source.push(')');
            groups.push(MatchGroup {
                offset,
                count: pattern.captures,
                kind: rule.action,
                rule_index,
            });
            offset += pattern.captures + 1;
        }
        let regex = rules
            .get(start)
            .map(|rule| Regex::with_flags(&source, patterns[rule.pattern.0].flags.as_str()))
            .transpose()?;
        Ok(Self { regex, groups })
    }

    /// Earliest match wins; alternative order breaks equal-position ties.
    fn search<'a>(&self, text: &'a str, position: usize) -> Option<Hit<'a>> {
        let found = self.regex.as_ref()?.find_from(text, position).next();
        #[cfg(test)]
        SEARCH_BYTES.with(|work| {
            work.set(
                work.get() + found.as_ref().map_or(text.len(), |m| m.range.end) - position + 1,
            );
        });
        let found = found?;
        let group = self
            .groups
            .iter()
            .find(|g| found.captures[g.offset].is_some())?;
        Some(Hit {
            index: found.range.start,
            end: found.range.end,
            text: &text[found.range],
            captures: found.captures[group.offset + 1..group.offset + 1 + group.count]
                .iter()
                .map(|r| r.as_ref().map(|r| &text[r.clone()]))
                .collect(),
            kind: group.kind,
            rule_index: group.rule_index,
        })
    }
}

/// Preserve character classes and escapes while relocating numeric references.
fn rewrite_backreferences(
    out: &mut String,
    source: &str,
    offset: usize,
    rewriter: &Regex,
) -> Result<(), Failure> {
    let mut end = 0;
    for token in rewriter.find_iter(source) {
        out.push_str(&source[end..token.range.start]);
        if let Some(number) = token.group(1) {
            write!(out, "\\{}", source[number].parse::<usize>()? + offset)?;
        } else {
            out.push_str(&source[token.range.clone()]);
        }
        end = token.range.end;
    }
    out.push_str(&source[end..]);
    Ok(())
}

/// Prepare full matchers and reachable ignored-begin suffixes for these roots.
pub(super) fn prepare(
    tables: &Tables,
    roots: impl IntoIterator<Item = ModeId>,
) -> Result<(), Failure> {
    let mut pending: Vec<_> = roots.into_iter().collect();
    let mut visited = vec![false; tables.modes.len()];
    while let Some(id) = pending.pop() {
        if std::mem::replace(&mut visited[id.0], true) {
            continue;
        }
        let mode = &tables.modes[id.0];
        published(mode.matcher.get_or_init(|| {
            MultiRegex::compile(&tables.sources, &mode.rules, 0, &tables.rewriter)
                .map_err(Arc::from)
        }))?;
        for pattern in [mode.end, mode.keyword_pattern].into_iter().flatten() {
            tables.prepare_pattern(pattern.0)?;
        }
        pending.extend(mode.starts);
        for (index, rule) in mode.rules.iter().enumerate() {
            let RuleKind::Begin(target) = rule.action else {
                continue;
            };
            pending.push(target);
            let target = &tables.modes[target.0];
            if index + 1 >= mode.begin_rule_count
                || !(super::callbacks::may_ignore(target.before)
                    || super::callbacks::may_ignore(target.on_begin))
            {
                continue;
            }
            published(rule.resumption.get_or_init(|| {
                MultiRegex::compile(&tables.sources, &mode.rules, index + 1, &tables.rewriter)
                    .map_err(Arc::from)
            }))?;
        }
    }
    Ok(())
}

/// Position and same-position ignored-begin resumption.
#[derive(Default)]
pub(super) struct Cursor<'a> {
    /// Current byte position in the original text.
    pub position: usize,
    /// Prepared suffix after an ignored begin, if any.
    pub resume: Option<&'a MultiRegex>,
    /// Previous transition for zero-width begin/end progress.
    pub last: Option<(RuleKind, usize)>,
}

impl Cursor<'_> {
    /// Resume remaining rules only at the ignored match's position.
    pub fn find<'a>(&self, mode: &Mode, text: &'a str) -> Option<Hit<'a>> {
        let matcher = mode.matcher.get()?.as_ref().ok()?;
        if self.position > text.len() {
            return None;
        }
        let hit = self.resume.unwrap_or(matcher).search(text, self.position);
        if self.resume.is_none() || hit.as_ref().is_some_and(|h| h.index == self.position) {
            return hit;
        }
        (self.position < text.len())
            .then(|| matcher.search(text, self.position + scalar_len(text, self.position)))
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
