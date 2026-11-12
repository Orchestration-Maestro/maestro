//! Item-local source selections and monotone continuation history.
use super::{
    Marker,
    record::{BodyLine, LineEnds, Shape},
};
use crate::{
    MarkedOptions,
    lexer::Child,
    rules::ItemStops,
    source::{Cursor, ScopeSource, Segmented},
};
use maestro_text::{is_whitespace_scalar, truncate_utf16};
use std::rc::Rc;

/// One recorded line an item admits as aligned.
struct Taken {
    /// Start of the line in the operand.
    start: usize,
    /// End of the line in the operand, terminator included.
    end: usize,
    /// Leading spaces dedented from the line.
    cut: usize,
    /// Spelling of the line once dedented.
    shape: Shape,
}

/// The last non-blank line of an admitted run and the history measured at it.
struct Boundary {
    /// Position of the line in the run.
    index: usize,
    /// Semantic body through the line's last non-whitespace scalar.
    body_end: usize,
    /// Raw through the same scalar.
    raw_end: usize,
    /// Whether a blank line precedes the boundary.
    blank: bool,
}

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

/// Added source pieces joined only when a reader needs the whole value.
struct Accumulated {
    /// Value of the pieces joined so far.
    value: Segmented,
    /// Admitted pieces awaiting that join.
    pending: Cursor,
    /// Trailing admitted piece, which a following piece extends while it continues the
    /// same selection, so adjacent lines of one allocation stay one piece.
    last: Option<ScopeSource>,
}

impl From<Segmented> for Accumulated {
    fn from(value: Segmented) -> Self {
        Self {
            value,
            pending: Cursor::default(),
            last: None,
        }
    }
}

impl From<ScopeSource> for Accumulated {
    fn from(source: ScopeSource) -> Self {
        Self {
            value: ScopeSource::identity("").into(),
            pending: Cursor::default(),
            last: Some(source),
        }
    }
}

impl Accumulated {
    /// Retain one admitted piece, extending the trailing piece when it continues it.
    fn push(&mut self, piece: ScopeSource) {
        if let Some(last) = &mut self.last
            && last.adjoin(&piece)
        {
            return;
        }
        if let Some(last) = self.last.replace(piece) {
            self.pending.append(last);
        }
    }

    /// Retain a shared splice after every piece admitted so far, without visiting its bytes.
    fn extend(&mut self, piece: Segmented) {
        if piece.len() == 0 {
            return;
        }
        if let Some(last) = self.last.take() {
            self.pending.append(last);
        }
        self.pending.segmented(piece);
    }

    /// Logical bytes accumulated so far.
    fn len(&self) -> usize {
        self.value.len() + self.pending.len() + self.last.as_ref().map_or(0, |last| last.text.len())
    }

    /// The whole value, joining the admitted pieces at most once.
    fn value(&mut self) -> &Segmented {
        if let Some(last) = self.last.take() {
            self.pending.append(last);
        }
        if !self.pending.is_empty() {
            let pending = std::mem::take(&mut self.pending);
            self.value.append(&pending.seal());
        }
        &self.value
    }

    /// The whole value, consuming the accumulator.
    fn into_value(mut self) -> Segmented {
        self.value();
        self.value
    }

    /// Discard a logical suffix after joining the admitted pieces.
    fn truncate(&mut self, end: usize) {
        self.value();
        self.value.truncate(end);
    }

    /// Charge the unique source this accumulated value keeps alive, joining the
    /// pieces it is still admitting once.
    #[cfg(test)]
    fn retained(&mut self, retained: &mut crate::tests::Retained) {
        self.value().retained(retained);
    }
}

/// One item's admission state, independent of its provisional block publication.
pub(super) struct ItemLines {
    /// Child invocation retained until the owning list's final publication.
    pub(super) child: Child,
    /// Admitted physical spelling, including trailing whitespace.
    raw: Accumulated,
    /// Dedented spelling selected from the same shared source.
    body: Accumulated,
    /// Each terminated line of `body`, the only places it holds a terminator.
    ends: Rc<Vec<BodyLine>>,
    /// Spelling of the final line of `body`, recorded once its terminator is admitted.
    shape: Shape,
    /// Final admitted line, whose terminator separates it from the next.
    tail: ScopeSource,
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
        let shape = Shape::of(&body.text);
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
            ends: Rc::default(),
            shape,
            tail: source.clone(),
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
        self.raw = source.select(start..offset).into();
        self.body.truncate(self.trimmed.body_end);
        let kept = self
            .ends
            .partition_point(|line| line.end <= self.trimmed.body_end);
        Rc::make_mut(&mut self.ends).truncate(kept);
        let padding_end = separator.text.len() - usize::from(separator.text.ends_with('\n'));
        let mut padding = separator.select(std::iter::once(0..padding_end));
        if matches!(self.trimmed.padding, Padding::Expanded) {
            padding = padding.replace("\t", "    ");
        }
        self.body.push(padding);
        self.shape = Shape::Other;
        self.tail = separator;
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
        let empty = original
            .text
            .trim_end_matches(is_whitespace_scalar)
            .is_empty();
        let aligned = empty || indented(&original.text, self.indent);
        // An aligned line starts past every stop rule's indentation, so only others test them.
        if !aligned && (self.rules.interrupts(&source.text) || self.blank || self.previous_stops) {
            return false;
        }
        let measured = original.replace("\t", "    ");
        let start = truncate_utf16(&measured.text, self.indent).len();
        self.separate();
        let content = if aligned {
            measured.select(std::iter::once(start..measured.text.len()))
        } else {
            original
        };
        let body_end = self.body.len() + content.text.trim_end_matches(is_whitespace_scalar).len();
        self.shape = Shape::of(&content.text);
        self.body.push(content);
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

    /// Admit, as one run, the aligned lines the enclosing item recorded from `offset` of
    /// `source`, returning the bytes taken.
    ///
    /// Each line gets the verdict `append` gives an aligned line, from its recorded
    /// spelling instead of its text; the first line that is not aligned, is spelled
    /// with a tab or reaches past the operand ends the run.
    pub(super) fn adopt(&mut self, source: &Segmented, lines: &LineEnds, offset: usize) -> usize {
        #[cfg(test)]
        if !crate::tests::adopting() {
            return 0;
        }
        if !matches!(self.phase, Phase::Open) {
            return 0;
        }
        let mut start = offset;
        let mut run = Vec::new();
        for (end, shape) in lines.recorded(offset, source.len()) {
            let Some((cut, shape)) = shape.dedent(self.indent) else {
                break;
            };
            let cut = match shape {
                Shape::Blank => (end - 1 - start).min(self.indent),
                _ => cut,
            };
            run.push(Taken {
                start,
                end,
                cut,
                shape,
            });
            start = end;
        }
        let Some(last) = run.last() else {
            return 0;
        };
        let taken = last.end - offset;
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.list_adopted += run.len());
        self.admit_run(source, &run);
        taken
    }

    /// Admit the lines of one run in the order and with the history `append` would give them.
    fn admit_run(&mut self, source: &Segmented, run: &[Taken]) {
        self.separate();
        let raw_before = self.raw.len();
        let origin = run[0].start;
        let mut text = None;
        for (index, line) in run.iter().enumerate() {
            let terminated = index + 1 < run.len();
            let piece = source.select(line.start + line.cut..line.end - usize::from(!terminated));
            let body_before = self.body.len();
            self.body.extend(piece);
            if terminated {
                Rc::make_mut(&mut self.ends).push(BodyLine {
                    end: self.body.len(),
                    shape: line.shape,
                });
            }
            match line.shape {
                Shape::Blank => self.blank = true,
                Shape::Text {
                    trimmed,
                    columns: _,
                } => {
                    text = Some(Boundary {
                        index,
                        body_end: body_before + trimmed,
                        raw_end: raw_before + line.start - origin + trimmed + line.cut,
                        blank: self.blank,
                    });
                }
                Shape::Other => {}
            }
        }
        self.raw
            .extend(source.select(origin..run[run.len() - 1].end));
        self.settle_run(source, run, text);
    }

    /// Record the history the final lines of an admitted run leave behind.
    fn settle_run(&mut self, source: &Segmented, run: &[Taken], text: Option<Boundary>) {
        let line = |index: usize| {
            let line = &run[index];
            source.select(line.start..line.end).materialize()
        };
        let last = run.len() - 1;
        let tail = line(last);
        let stops = |line: &ScopeSource, cut: usize| {
            self.rules.ends_lazy(&line.text[cut..line.text.len() - 1])
        };
        self.previous_stops = stops(&tail, run[last].cut);
        if let Some(boundary) = text {
            let kept = if boundary.index == last {
                tail.clone()
            } else {
                line(boundary.index)
            };
            self.raw_end = boundary.raw_end;
            self.trimmed = History {
                body_end: boundary.body_end,
                blank: boundary.blank,
                previous_stops: stops(&kept, run[boundary.index].cut),
                phase: Phase::Open,
                line: kept,
                padding: Padding::Expanded,
            };
        }
        let before = if last == 0 {
            self.suffix[1].clone()
        } else {
            line(last - 1)
        };
        self.suffix = [before, tail.clone()];
        self.shape = run[last].shape;
        self.tail = tail;
    }

    /// Admit the terminator of the final line and record that line ahead of the next one.
    fn separate(&mut self) {
        if let Some(last) = self.tail.text.len().checked_sub(1) {
            self.body
                .push(self.tail.select(std::iter::once(last..last + 1)));
            if self.tail.text.ends_with('\n') {
                Rc::make_mut(&mut self.ends).push(BodyLine {
                    end: self.body.len(),
                    shape: self.shape,
                });
            }
        }
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
        self.tail = source.clone();
        self.raw.push(source);
    }

    /// Charge the unique source this item keeps alive while its list is open.
    ///
    /// Both accumulators join their pending pieces first, as a reader of the whole
    /// value does.
    #[cfg(test)]
    pub(super) fn retained(&mut self, retained: &mut crate::tests::Retained) {
        self.raw.retained(retained);
        self.body.retained(retained);
        self.trimmed.line.retained(retained);
        for line in &self.suffix {
            line.retained(retained);
        }
        self.child.retained(retained);
    }

    /// Keep only what publication and a caller replay read from a stopped item.
    pub(super) fn complete(self) -> CompletedItem {
        let body = self.body.into_value();
        let task = super::task(&body).1;
        let blank_end = blank_suffix(&self.suffix);
        CompletedItem {
            child: self.child,
            body,
            task,
            blank_end,
        }
    }

    /// Borrow dedented source for child admission.
    pub(super) fn body(&mut self) -> &Segmented {
        self.body.value()
    }

    /// The body's line ends, for an operand that starts `base` bytes into the body.
    pub(super) fn line_ends(&self, base: usize) -> LineEnds {
        LineEnds {
            ends: Rc::clone(&self.ends),
            base,
        }
    }

    /// Final semantic boundary already measured during physical-line admission.
    pub(super) fn body_end(&self) -> usize {
        self.trimmed.body_end
    }

    /// Borrow the complete recognized raw, independently of final-list trim.
    pub(super) fn raw(&mut self) -> &Segmented {
        self.raw.value()
    }

    /// End of the final non-whitespace raw scalar.
    pub(super) fn raw_end(&self) -> usize {
        self.raw_end
    }
}

/// Whether a line's leading spaces and tabs, each tab counting four columns as alignment
/// expands it, reach `indent` columns; only that leading run is read.
fn indented(line: &str, indent: usize) -> bool {
    let mut columns = 0;
    for byte in line.bytes() {
        if columns >= indent {
            return true;
        }
        columns += match byte {
            b' ' => 1,
            b'\t' => 4,
            _ => return false,
        };
    }
    columns >= indent
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
    /// Dedented semantic body a caller replay re-admits.
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
    pub(super) fn retained(&mut self, retained: &mut crate::tests::Retained) {
        self.body.retained(retained);
        self.child.retained(retained);
    }
}

#[cfg(test)]
mod tests;
