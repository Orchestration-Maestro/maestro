//! Container-local source framing.
use crate::inline::ScopeKey;
use std::{cell::RefCell, collections::HashMap, ops::Range};
/// Authored source and the native containers enclosing the current events.
#[derive(Clone, Copy)]
pub(crate) struct Source<'a> {
    /// Normalized original input.
    pub(crate) text: &'a str,
    /// Container prefixes removed by the native parser on continuation lines.
    pub(crate) frame: Option<&'a Frame<'a>>,
    /// Whether the events belong to a table cell, where `\|` spells a literal pipe.
    pub(crate) cell: bool,
    /// Document-local scope collector; absent during local interpretation.
    pub(crate) scopes: Option<&'a RefCell<HashMap<ScopeKey, ScopeSource>>>,
}
/// One enclosing native block's continuation prefix.
pub(crate) struct Frame<'a> {
    /// Outer block, stripped before this block.
    pub(crate) parent: Option<&'a Frame<'a>>,
    /// Quote marker or list continuation indentation.
    pub(crate) kind: Prefix,
}
/// Native block framing relevant to authored inline spans.
pub(crate) enum Prefix {
    /// One quote marker with its optional following space.
    Quote,
    /// List marker width on the item's first line.
    Item(usize),
}
impl Frame<'_> {
    /// Removes only prefixes belonging to the captured container path.
    pub(crate) fn strip<'a>(&self, line: &'a str) -> &'a str {
        let line = self.parent.map_or(line, |parent| parent.strip(line));
        match self.kind {
            Prefix::Quote => strip_quote(line),
            Prefix::Item(width) => {
                let spaces = line.bytes().take_while(|byte| *byte == b' ').count();
                &line[spaces.min(width)..]
            }
        }
    }
}
impl Source<'_> {
    /// Retains authored bytes while removing enclosing continuation prefixes.
    pub(crate) fn authored(&self, range: Range<usize>) -> String {
        let authored = self.text[range]
            .split('\n')
            .enumerate()
            .map(|(index, line)| {
                if index == 0 {
                    line
                } else {
                    self.frame.map_or(line, |frame| frame.strip(line))
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        self.spelling(authored)
    }

    /// Spells a table cell's `\|` as a literal pipe; other sources pass through unchanged.
    pub(crate) fn spelling(&self, text: String) -> String {
        if self.cell {
            text.replace("\\|", "|")
        } else {
            text
        }
    }
}
/// Removes a native quote prefix, leaving lazy continuation text intact.
pub(crate) fn strip_quote(line: &str) -> &str {
    let spaces = line.bytes().take_while(|byte| *byte == b' ').count().min(3);
    line[spaces..]
        .strip_prefix('>')
        .map_or(line, |line| line.strip_prefix(' ').unwrap_or(line))
}

/// Container-local authored source with ordered native provenance.
pub(crate) struct ScopeSource {
    /// Bytes before inline interpretation.
    pub(crate) text: String,
    /// Ordered identity spans or atomic table-pipe transformations.
    units: Vec<(Range<usize>, Range<usize>)>,
}
impl ScopeSource {
    /// Project a native operand through captured framing, not string-offset subtraction.
    pub(crate) fn local(&self, range: &Range<usize>) -> Option<Range<usize>> {
        self.project(range, false)
    }
    /// Project an authored selection back through the same captured provenance.
    pub(crate) fn native(&self, range: &Range<usize>) -> Option<Range<usize>> {
        self.project(range, true)
    }
    /// Bytes remaining in the current identity span or atomic transformed unit.
    pub(crate) fn remaining(&self, offset: usize) -> usize {
        let index = self.units.partition_point(|(_, local)| local.end <= offset);
        self.units
            .get(index)
            .map_or(0, |(_, local)| local.end - offset)
    }
    /// Share boundary projection in both directions, retaining atomic transformations.
    fn project(&self, range: &Range<usize>, reverse: bool) -> Option<Range<usize>> {
        let pair = |index| {
            let (native, local) = self.units.get(index)?;
            Some(if reverse {
                (local, native)
            } else {
                (native, local)
            })
        };
        let first = self.units.partition_point(|(native, local)| {
            if reverse {
                local.end <= range.start
            } else {
                native.end <= range.start
            }
        });
        let last = self
            .units
            .partition_point(|(native, local)| {
                if reverse {
                    local.start < range.end
                } else {
                    native.start < range.end
                }
            })
            .checked_sub(1)?;
        let (from, to) = pair(first)?;
        let start = to.start
            + if from.len() == to.len() {
                range.start.saturating_sub(from.start)
            } else {
                0
            };
        let (from, to) = pair(last)?;
        let end = to.end
            - if from.len() == to.len() {
                from.end.saturating_sub(range.end)
            } else {
                0
            };
        Some(start..end)
    }
}
impl Source<'_> {
    /// Capture inline provenance while the native container path is still available.
    pub(crate) fn scope(&self, key: &Range<usize>, range: Range<usize>, label: bool) {
        let Some(scopes) = self.scopes else {
            return;
        };
        let mut mapped = ScopeSource {
            text: String::new(),
            units: Vec::new(),
        };
        let mut native = range.start;
        for (index, line) in self.text[range].split_inclusive('\n').enumerate() {
            let shown = if index == 0 {
                line
            } else {
                self.frame.map_or(line, |frame| frame.strip(line))
            };
            let begin = native + line.len() - shown.len();
            mapped.push(shown, begin, self.cell);
            native += line.len();
        }
        scopes
            .borrow_mut()
            .insert((key.start, key.end, label), mapped);
    }
}
impl ScopeSource {
    /// Record equal spans and the table parser's one atomic spelling transform.
    fn push(&mut self, mut text: &str, mut native: usize, cell: bool) {
        while !text.is_empty() {
            let end = if cell {
                text.find("\\|").unwrap_or(text.len())
            } else {
                text.len()
            };
            let size = if end == 0 { 2 } else { end };
            let shown = if end == 0 { "|" } else { &text[..end] };
            let start = self.text.len();
            self.text.push_str(shown);
            self.units
                .push((native..native + size, start..self.text.len()));
            native += size;
            text = &text[size..];
        }
    }
}
