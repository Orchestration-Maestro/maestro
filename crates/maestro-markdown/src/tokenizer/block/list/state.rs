//! Item-local source selections and monotone continuation history.
use super::Marker;
use crate::{
    MarkedOptions,
    lexer::Child,
    rules::ItemStops,
    source::{ScopeSource, Segmented},
};
use maestro_text::{is_whitespace_scalar, truncate_utf16};

/// History at the last non-whitespace raw boundary, before final-list trimming.
#[derive(Clone)]
struct History {
    /// Semantic body through its last non-whitespace scalar.
    body_end: usize,
    /// Whether a blank line precedes that boundary.
    blank: bool,
    /// Whether that line excludes another lazy continuation.
    previous_stops: bool,
    /// Admission phase at the trimmed boundary.
    phase: Phase,
    /// Last nonblank physical line before discarded separators.
    line: ScopeSource,
    /// Whether aligned continuation tabs were expanded.
    padding: Padding,
}

/// Empty markers have one special second-line boundary.
#[derive(Clone, Copy)]
enum Phase {
    /// An empty marker can consume one following blank line.
    Empty,
    /// Ordinary continuation history applies.
    Open,
    /// The special blank second line completed this item.
    Finished,
}

/// Spelling of padding retained on the final nonblank line.
#[derive(Clone, Copy)]
enum Padding {
    /// First-line and lazy tabs retain their original spelling.
    Authored,
    /// Aligned continuation tabs expand to four spaces.
    Expanded,
}

/// One item's admission state, independent of its provisional block publication.
pub(super) struct ItemLines {
    /// Child invocation retained until the owning list's final publication.
    pub(super) child: Child,
    /// Admitted physical spelling, including trailing whitespace.
    raw: Segmented,
    /// Dedented spelling selected from the same shared source.
    body: Segmented,
    /// Raw end before trailing whitespace, which the list trims from its final item.
    raw_end: usize,
    /// Whether an admitted line has been blank.
    blank: bool,
    /// Whether the previous line excludes another lazy continuation.
    previous_stops: bool,
    /// Whether empty-marker or ordinary continuation rules apply.
    phase: Phase,
    /// Continuation indentation selected by the marker.
    indent: usize,
    /// Lookahead predicates shared by every item at this indentation and grammar.
    rules: &'static ItemStops,
    /// History restored when trimmed raw is resumed.
    trimmed: History,
    /// The two most recent lines suffice for the blank-suffix predicate.
    suffix: [ScopeSource; 2],
}

impl ItemLines {
    /// Admit the marker's first physical line without flattening later input.
    pub(super) fn new(source: &ScopeSource, marker: &Marker, options: MarkedOptions) -> Self {
        let end = marker.line - usize::from(source.text[..marker.line].ends_with('\n'));
        let mut first = source.select(std::iter::once(marker.prefix..end));
        if marker.tabs > 0 {
            first = first.replace_prefix(marker.tabs, &"   ".repeat(marker.tabs));
        }
        let blank = first.text.trim_matches(is_whitespace_scalar).is_empty();
        let start = if blank {
            first.text.len()
        } else {
            marker.indent - marker.prefix
        };
        let body = first.select(std::iter::once(start..first.text.len()));
        let rules = crate::rules::item_stops(marker.indent, options);
        let previous_stops = rules.ends_lazy(&first.text);
        let trimmed = History {
            body_end: body.text.trim_end_matches(is_whitespace_scalar).len(),
            blank,
            previous_stops,
            phase: if blank { Phase::Empty } else { Phase::Open },
            line: source.clone(),
            padding: Padding::Authored,
        };
        Self {
            child: Child::default(),
            raw: source.clone().into(),
            body: body.into(),
            raw_end: source.text.trim_end_matches(is_whitespace_scalar).len(),
            blank,
            previous_stops,
            phase: if blank { Phase::Empty } else { Phase::Open },
            indent: marker.indent,
            rules,
            trimmed,
            suffix: [ScopeSource::identity(""), source.clone()],
        }
    }

    /// Restore the final-list trim before supplying the next physical line.
    pub(super) fn resume(&mut self, source: &Segmented, start: usize) -> usize {
        let end = start + self.raw_end;
        let mut pending = crate::source::Cursor::default();
        pending.segmented(source.select(end..source.len()));
        let separator = pending.line().map_or_else(
            || ScopeSource::identity(""),
            crate::source::Cursor::materialize,
        );
        let offset = end + separator.text.len();
        self.raw = source.select(start..offset);
        self.body.truncate(self.trimmed.body_end);
        let padding_end = separator.text.len() - usize::from(separator.text.ends_with('\n'));
        let mut padding = separator.select(std::iter::once(0..padding_end));
        if matches!(self.trimmed.padding, Padding::Expanded) {
            padding = padding.replace("\t", "    ");
        }
        self.body.append(&padding.into());
        self.blank = self.trimmed.blank;
        self.previous_stops = self.trimmed.previous_stops;
        self.phase = self.trimmed.phase;
        self.suffix = [ScopeSource::identity(""), self.trimmed.line.clone()];
        offset
    }

    /// Admit one continuation through the existing interruption and indentation rules.
    pub(super) fn append(&mut self, source: &ScopeSource) -> bool {
        if matches!(self.phase, Phase::Finished) {
            return false;
        }
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.list_lines += 1);
        let end = source.text.len() - usize::from(source.text.ends_with('\n'));
        let original = source.select(std::iter::once(0..end));
        if matches!(self.phase, Phase::Empty) && original.text.trim_matches([' ', '\t']).is_empty()
        {
            self.retain(source.clone(), self.trimmed.body_end, Padding::Authored);
            self.phase = Phase::Finished;
            return true;
        }
        if self.rules.interrupts(&source.text) {
            return false;
        }
        let measured = original.replace("\t", "    ");
        let spaces = measured
            .text
            .bytes()
            .take_while(|byte| *byte == b' ')
            .count();
        let empty = original.text.trim_matches(is_whitespace_scalar).is_empty();
        let aligned = spaces >= self.indent || empty;
        if !aligned && (self.blank || self.previous_stops) {
            return false;
        }
        let start = truncate_utf16(&measured.text, self.indent).len();
        self.body
            .append(&self.raw.select(self.raw.len() - 1..self.raw.len()));
        let content = if aligned {
            measured.select(std::iter::once(start..measured.text.len()))
        } else {
            original
        };
        let body_end = self.body.len() + content.text.trim_end_matches(is_whitespace_scalar).len();
        self.body.append(&content.into());
        self.previous_stops = self.rules.ends_lazy(&measured.text[start..]);
        self.blank |= empty;
        self.phase = Phase::Open;
        self.retain(
            source.clone(),
            body_end,
            if aligned {
                Padding::Expanded
            } else {
                Padding::Authored
            },
        );
        true
    }

    /// Retain admitted raw and its trimmed boundary without scanning the growing prefix.
    fn retain(&mut self, source: ScopeSource, body_end: usize, padding: Padding) {
        let end = source.text.trim_end_matches(is_whitespace_scalar).len();
        if end > 0 {
            self.raw_end = self.raw.len() + end;
            self.trimmed = History {
                body_end,
                blank: self.blank,
                previous_stops: self.previous_stops,
                phase: self.phase,
                line: source.clone(),
                padding,
            };
        }
        self.suffix.swap(0, 1);
        self.suffix[1] = source.clone();
        self.raw.append(&source.into());
    }

    /// Charge the unique source this item keeps alive while its list is open.
    #[cfg(test)]
    pub(super) fn retained(&self, retained: &mut crate::tests::Retained) {
        self.raw.retained(retained);
        self.body.retained(retained);
        self.trimmed.line.retained(retained);
        for line in &self.suffix {
            line.retained(retained);
        }
    }

    /// Keep only what publication and a caller replay read from a stopped item.
    pub(super) fn complete(self) -> CompletedItem {
        let task = super::task(&self.body).1;
        let blank_end = blank_suffix(&self.suffix);
        CompletedItem {
            child: self.child,
            body: self.body,
            task,
            blank_end,
        }
    }

    /// Borrow dedented source for child admission.
    pub(super) fn body(&self) -> &Segmented {
        &self.body
    }

    /// Final semantic boundary already measured during physical-line admission.
    pub(super) fn body_end(&self) -> usize {
        self.trimmed.body_end
    }

    /// Borrow the complete recognized raw, independently of final-list trim.
    pub(super) fn raw(&self) -> &Segmented {
        &self.raw
    }

    /// End of the final non-whitespace raw scalar.
    pub(super) fn raw_end(&self) -> usize {
        self.raw_end
    }
}

/// Whether the two physical lines that end an item carry a blank-line suffix.
fn blank_suffix(window: &[ScopeSource; 2]) -> bool {
    let window = [&*window[0].text, &*window[1].text].concat();
    #[cfg(test)]
    crate::tests::block_operand(&window);
    crate::rules::LIST_BLANK_END
        .as_ref()
        .is_some_and(|rule| rule.find(&window).is_some())
}

/// One stopped item as its list retains it, until publication and caller replay.
pub(super) struct CompletedItem {
    /// Settled child admission: its published output and the step a caller replay resumes.
    child: Child,
    /// Dedented semantic body, which publication reads the item's task marker from.
    body: Segmented,
    /// Task marker state, consumed when the child publishes.
    task: Option<bool>,
    /// Whether a blank line ends this item, which loosens the list.
    blank_end: bool,
}

impl CompletedItem {
    /// Borrow the dedented semantic body a caller replay re-admits.
    pub(super) fn body(&self) -> &Segmented {
        &self.body
    }

    /// Borrow the child invocation a caller replay re-admits through.
    pub(super) fn child(&mut self) -> &mut Child {
        &mut self.child
    }

    /// Publish the item's settled output, inserting its task marker once.
    pub(super) fn finish(&mut self) -> crate::lexer::BlockTokens {
        self.child.finish(self.task)
    }

    /// Whether a blank line ends this item, which loosens the list.
    pub(super) fn blank_end(&self) -> bool {
        self.blank_end
    }

    /// Charge the unique source this item keeps alive until its list publishes.
    #[cfg(test)]
    pub(super) fn retained(&self, retained: &mut crate::tests::Retained) {
        self.body.retained(retained);
    }
}

#[cfg(test)]
mod tests;
