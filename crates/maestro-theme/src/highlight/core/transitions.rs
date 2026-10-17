//! Mode entry, ending, resumption and scalar-safe EOF handling.
use super::{
    IllegalPolicy, Run,
    callbacks::{admit, may_ignore},
    matcher::{Cursor, Hit},
    types::{Boundary, Frame, ModeId, ParentPolicy, Processing, RuleKind, Scope, SubLanguage},
};

impl<'a> Run<'a> {
    /// Parse one complete buffer, returning whether an illegal rule rejected it.
    pub(super) fn scan(&mut self, text: &str, illegals: IllegalPolicy) -> bool {
        let mut cursor = Cursor::default();
        while let Some(hit) = cursor.find(self.mode(), text) {
            cursor.resume = None;
            self.buffer.push_str(&text[cursor.position..hit.index]);
            if matches!(cursor.last, Some((RuleKind::Begin(_), position)) if position == hit.index)
                && hit.kind == RuleKind::End
                && hit.text.is_empty()
            {
                cursor.advance(text, hit.index, &mut self.buffer);
                continue;
            }
            cursor.last = Some((hit.kind, hit.index));
            match hit.kind {
                RuleKind::Begin(next) => self.begin(next, &hit, text, &mut cursor),
                RuleKind::End => self.end(&hit, text, &mut cursor),
                RuleKind::Illegal if illegals == IllegalPolicy::Reject => return true,
                RuleKind::Illegal
                    if hit.text.is_empty()
                        && self.engine.profile == super::super::HighlightProfile::CodeBlock =>
                {
                    self.buffer.push('\n');
                    cursor.skip(text, hit.index);
                }
                RuleKind::Illegal if hit.text.is_empty() => cursor.skip(text, hit.index),
                RuleKind::Illegal => {
                    self.buffer.push_str(hit.text);
                    cursor.position = hit.end;
                }
            }
        }
        self.buffer
            .push_str(text.get(cursor.position..).unwrap_or_default());
        self.flush();
        false
    }

    /// Admit a begin, preserving remaining same-position rules when rejected.
    fn begin(&mut self, next: ModeId, hit: &Hit<'_>, text: &str, cursor: &mut Cursor<'a>) {
        let mode = &self.engine.tables.modes[next.0];
        let mut delimiter = None;
        if !admit(mode.before, hit, text, &mut delimiter)
            || !admit(mode.on_begin, hit, text, &mut delimiter)
        {
            if may_ignore(mode.before) || may_ignore(mode.on_begin) {
                cursor.resume = self.engine.tables.modes[self.stack.current.mode.0].rules
                    [hit.rule_index]
                    .resumption
                    .get()
                    .and_then(|matcher| matcher.as_ref().ok());
            }
            if cursor.resume.is_some() {
                cursor.position = hit.index;
            } else {
                cursor.advance(text, hit.index, &mut self.buffer);
            }
            return;
        }
        if mode.processing == Processing::Skip {
            self.buffer.push_str(hit.text);
        } else {
            if mode.begin == Boundary::Exclude {
                self.buffer.push_str(hit.text);
            }
            self.flush();
            if mode.begin == Boundary::Include {
                hit.text.clone_into(&mut self.buffer);
            }
        }
        self.enter(next, hit, delimiter);
        cursor.position = if mode.begin == Boundary::Return {
            hit.index
        } else {
            hit.end
        };
    }

    /// Enter one immutable mode with a newly owned callback slot.
    fn enter(&mut self, mode: ModeId, hit: &Hit<'_>, delimiter: Option<String>) {
        let scope = &self.engine.tables.modes[mode.0].begin_scope;
        match scope {
            Scope::Wrapped => self.buffer.clear(),
            Scope::Captures(_) => {
                self.captures(scope, hit);
                self.buffer.clear();
            }
            Scope::Plain => {}
        }
        self.stack.enter(Frame { mode, delimiter });
    }

    /// Find an admitted end, inheriting parent terminators only when requested.
    fn ending_depth(&mut self, hit: &Hit<'_>, text: &str) -> Option<usize> {
        let tables = self.engine.tables;
        let mut matched = None;
        for (depth, frame) in std::iter::once(&mut self.stack.current)
            .chain(self.stack.parents.iter_mut().rev())
            .enumerate()
        {
            let mode = &tables.modes[frame.mode.0];
            let ends = mode.end.is_some_and(|pattern| {
                tables.patterns[pattern.0]
                    .get()
                    .and_then(|pattern| pattern.as_ref().ok())
                    .and_then(|pattern| pattern.find_from(text, hit.index).next())
                    .is_some_and(|found| found.range.start == hit.index)
            });
            if ends && admit(mode.on_end, hit, text, &mut frame.delimiter) {
                matched = Some(depth);
                break;
            }
            if !matches!(mode.parent, ParentPolicy::WithParent | ParentPolicy::Both) {
                break;
            }
        }
        let depth = matched?;
        let extra = std::iter::once(&self.stack.current)
            .chain(self.stack.parents.iter().rev())
            .skip(depth)
            .take(self.stack.parents.len() - depth)
            .take_while(|frame| {
                matches!(
                    tables.modes[frame.mode.0].parent,
                    ParentPolicy::EndsParent | ParentPolicy::Both
                )
            })
            .count();
        Some(depth + extra)
    }

    /// End the admitted frame chain and optionally enter its successor mode.
    fn end(&mut self, hit: &Hit<'_>, text: &str, cursor: &mut Cursor<'_>) {
        let Some(depth) = self.ending_depth(hit, text) else {
            self.buffer.push_str(hit.text);
            cursor.position = hit.end;
            return;
        };
        let mode = &self.engine.tables.modes[self.stack.current.mode.0];
        let boundary = mode.end_boundary;
        self.end_buffer(hit);
        let ended = std::iter::once(&self.stack.current)
            .chain(self.stack.parents.iter().rev())
            .nth(depth);
        let starts = ended.and_then(|frame| self.engine.tables.modes[frame.mode.0].starts);
        for _ in 0..=depth {
            let mode = &self.engine.tables.modes[self.stack.current.mode.0];
            if mode.processing != Processing::Skip && matches!(mode.sub, SubLanguage::None) {
                self.relevance += mode.relevance;
            }
            if let Some(parent) = self.stack.parents.pop() {
                self.stack.current = parent;
            }
        }
        if let Some(starts) = starts {
            self.enter(starts, hit, None);
        }
        cursor.position = if boundary == Boundary::Return {
            hit.index
        } else {
            hit.end
        };
    }

    /// Flush each end delimiter according to its scope and ownership policy.
    fn end_buffer(&mut self, hit: &Hit<'_>) {
        let mode = &self.engine.tables.modes[self.stack.current.mode.0];
        match &mode.end_scope {
            Scope::Wrapped => self.flush(),
            scope @ Scope::Captures(_) => {
                self.flush();
                self.captures(scope, hit);
            }
            Scope::Plain if mode.processing == Processing::Skip => self.buffer.push_str(hit.text),
            Scope::Plain => {
                if mode.end_boundary == Boundary::Include {
                    self.buffer.push_str(hit.text);
                }
                self.flush();
                if mode.end_boundary == Boundary::Exclude {
                    hit.text.clone_into(&mut self.buffer);
                }
            }
        }
    }
}
