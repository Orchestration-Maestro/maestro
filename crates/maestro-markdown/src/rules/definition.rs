//! Incremental definition grammar with ordered destination and title alternatives.
use crate::MarkedOptions;
use std::ops::Range;

/// One definition's captures in its complete local operand.
#[derive(Clone, Default)]
pub(crate) struct Definition {
    /// Authored reference identity without brackets.
    pub(crate) label: Range<usize>,
    /// Authored destination, including angle delimiters when present.
    pub(crate) destination: Range<usize>,
    /// Optional authored title, including its delimiters.
    pub(crate) title: Option<Range<usize>>,
    /// End of the recognized raw spelling.
    pub(crate) consumed: usize,
}

/// Whether appending source can still change the definition result.
#[derive(Clone)]
pub(crate) enum Verdict {
    /// An earlier grammar alternative can still succeed.
    NeedMore,
    /// No additional source can change this result.
    Stable(Option<Definition>),
}

/// A definition's grammar position, independent of capture offsets.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Up to three authored leading spaces.
    Indent(u8),
    /// Label content, recording whether it contains a non-whitespace scalar.
    Label(bool),
    /// One escaped label scalar.
    LabelEscape,
    /// Colon immediately following the label.
    Colon,
    /// Spaces following the colon.
    Head,
    /// One optional line break before the destination.
    HeadIndent,
    /// Unbracketed destination content.
    Destination,
    /// Angle destination content; each closing angle tries the suffix first.
    Angle,
    /// Optional title or plain definition termination.
    Suffix,
    /// Same-line title separation.
    Separator,
    /// Indentation after the title separator's single line break.
    TitleIndent,
    /// Double-quoted title content.
    Double,
    /// A backslash with an optional quote in a double-quoted title.
    DoubleEscape,
    /// Single-quoted title content.
    Single,
    /// A nonempty continuation line or the final line break of a single title.
    SingleLine,
    /// Parenthesized title content.
    Parenthesized,
    /// Trailing ASCII spaces, followed by a line break or the final input end.
    Spaces,
    /// Greedily consumed trailing line breaks.
    Newlines,
    /// A successful prefix retained behind higher-priority alternatives.
    Accepted,
}

/// One still-viable destination/title alternative and its capture offsets.
#[derive(Clone)]
struct Candidate {
    /// Next grammar position.
    phase: Phase,
    /// Captures accumulated along this alternative.
    matched: Definition,
}

/// Retained definition progress; no authored bytes are copied or revisited.
pub(crate) struct Recognizer {
    /// Ordered viable grammar alternatives, with at most one per position.
    candidates: Vec<Candidate>,
    /// Reused storage for the next scalar's viable alternatives.
    next: Vec<Candidate>,
    /// Total supplied UTF-8 bytes examined so far.
    cursor: usize,
    /// Whether label escapes may include line terminators.
    multiline_escape: bool,
    /// Result once all higher-priority alternatives have finished.
    stable: Verdict,
}

impl Recognizer {
    /// Begin the caller's label policy at the beginning of an operand.
    pub(crate) fn new(options: MarkedOptions) -> Self {
        Self {
            candidates: vec![Candidate {
                phase: Phase::Indent(0),
                matched: Definition::default(),
            }],
            next: Vec::new(),
            cursor: 0,
            multiline_escape: matches!(
                options,
                MarkedOptions::Browser { .. } | MarkedOptions::Documentation
            ),
            stable: Verdict::NeedMore,
        }
    }

    /// Extend the operand without restarting any earlier grammar work.
    pub(crate) fn extend(&mut self, source: &str) -> Verdict {
        if matches!(self.stable, Verdict::Stable(_)) {
            return self.stable.clone();
        }
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.definition_bytes += source.len());
        for scalar in source.chars() {
            self.advance(scalar);
            if self.candidates.is_empty() || self.candidates[0].phase == Phase::Accepted {
                return self.settle(self.candidates.first().map(|item| item.matched.clone()));
            }
        }
        Verdict::NeedMore
    }

    /// Resolve pending alternatives at the caller-owned end of the operand.
    pub(crate) fn finish(&mut self) -> Option<Definition> {
        if let Verdict::Stable(result) = &self.stable {
            return result.clone();
        }
        let result = self
            .candidates
            .iter()
            .find_map(|candidate| candidate.finish(self.cursor));
        self.stable = Verdict::Stable(result.clone());
        result
    }

    /// Examine one new scalar with a fixed set of grammar positions.
    fn advance(&mut self, scalar: char) {
        let position = self.cursor..self.cursor + scalar.len_utf8();
        for candidate in self.candidates.drain(..) {
            if candidate.phase == Phase::LabelEscape && !self.multiline_escape && line_end(scalar) {
                continue;
            }
            candidate.advance(scalar, position.clone(), &mut self.next);
        }
        self.cursor = position.end;
        std::mem::swap(&mut self.candidates, &mut self.next);
        self.next.clear();
    }

    /// Retain a final result for repeated observation without further admission.
    fn settle(&mut self, result: Option<Definition>) -> Verdict {
        self.stable = Verdict::Stable(result);
        self.stable.clone()
    }
}

/// Recognize a complete caller operand through the same retained grammar.
pub(crate) fn recognize(source: &str, options: MarkedOptions) -> Option<Definition> {
    let mut recognizer = Recognizer::new(options);
    for (offset, scalar) in source.char_indices() {
        if let Verdict::Stable(result) =
            recognizer.extend(&source[offset..offset + scalar.len_utf8()])
        {
            return result;
        }
    }
    recognizer.finish()
}

impl Candidate {
    /// Advance one grammar alternative by one new source scalar.
    fn advance(self, scalar: char, position: Range<usize>, output: &mut Vec<Self>) {
        match self.phase {
            Phase::Indent(_)
            | Phase::Label(_)
            | Phase::LabelEscape
            | Phase::Colon
            | Phase::Head
            | Phase::HeadIndent => self.head(scalar, position, output),
            Phase::Destination | Phase::Angle | Phase::Suffix => {
                self.destination(scalar, position, output);
            }
            Phase::Separator | Phase::TitleIndent => self.separator(scalar, position, output),
            Phase::Double
            | Phase::DoubleEscape
            | Phase::Single
            | Phase::SingleLine
            | Phase::Parenthesized => self.title(scalar, position, output),
            Phase::Spaces | Phase::Newlines => self.ending(scalar, position, output),
            Phase::Accepted => retain(output, self),
        }
    }

    /// Admit the indentation, escaped label and destination prelude.
    fn head(mut self, scalar: char, position: Range<usize>, output: &mut Vec<Self>) {
        match self.phase {
            Phase::Indent(count) if scalar == ' ' && count < 3 => {
                self.phase = Phase::Indent(count + 1);
            }
            Phase::Indent(_) if scalar == '[' => {
                self.matched.label.start = position.end;
                self.phase = Phase::Label(false);
            }
            Phase::Label(valid) if scalar == ']' && valid => {
                self.matched.label.end = position.start;
                self.phase = Phase::Colon;
            }
            Phase::Label(_) if scalar == '\\' => self.phase = Phase::LabelEscape,
            Phase::Label(valid) if !matches!(scalar, '[' | ']') => {
                self.phase = Phase::Label(valid || !maestro_text::is_whitespace_scalar(scalar));
            }
            Phase::LabelEscape => self.phase = Phase::Label(true),
            Phase::Colon if scalar == ':' => self.phase = Phase::Head,
            Phase::Head if scalar == ' ' => {}
            Phase::Head if scalar == '\n' => self.phase = Phase::HeadIndent,
            Phase::HeadIndent if matches!(scalar, ' ' | '\t') => {}
            Phase::Head | Phase::HeadIndent => {
                self.begin_destination(scalar, position, output);
                return;
            }
            _ => return,
        }
        retain(output, self);
    }

    /// Select bracketed or unbracketed destination syntax.
    fn begin_destination(mut self, scalar: char, position: Range<usize>, output: &mut Vec<Self>) {
        self.matched.destination.start = position.start;
        if scalar == '<' {
            self.phase = Phase::Angle;
        } else if maestro_text::is_whitespace_scalar(scalar) {
            return;
        } else {
            self.phase = Phase::Destination;
        }
        retain(output, self);
    }

    /// Retain the lazy angle alternative without re-reading rejected suffixes.
    fn destination(mut self, scalar: char, position: Range<usize>, output: &mut Vec<Self>) {
        match self.phase {
            Phase::Destination if !maestro_text::is_whitespace_scalar(scalar) => {
                retain(output, self);
            }
            Phase::Destination => {
                self.matched.destination.end = position.start;
                self.suffix(scalar, position, output);
            }
            Phase::Angle if scalar == '>' => {
                let mut closed = self.clone();
                closed.matched.destination.end = position.end;
                closed.phase = Phase::Suffix;
                retain(output, closed);
                retain(output, self);
            }
            Phase::Angle if !line_end(scalar) => retain(output, self),
            Phase::Suffix => self.suffix(scalar, position, output),
            _ => {}
        }
    }

    /// Prefer an optional title over the otherwise valid plain termination.
    fn suffix(self, scalar: char, position: Range<usize>, output: &mut Vec<Self>) {
        if matches!(scalar, ' ' | '\n') {
            let mut title = self.clone();
            title.phase = if scalar == ' ' {
                Phase::Separator
            } else {
                Phase::TitleIndent
            };
            retain(output, title);
        }
        self.ending(scalar, position, output);
    }

    /// Admit exactly one separator line break and its spaces or tabs.
    fn separator(mut self, scalar: char, position: Range<usize>, output: &mut Vec<Self>) {
        match (self.phase, scalar) {
            (Phase::Separator, ' ') | (Phase::TitleIndent, ' ' | '\t') => {}
            (Phase::Separator, '\n') => self.phase = Phase::TitleIndent,
            (_, '"' | '\'' | '(') => {
                self.matched.title = Some(position.start..position.end);
                self.phase = match scalar {
                    '"' => Phase::Double,
                    '\'' => Phase::Single,
                    _ => Phase::Parenthesized,
                };
            }
            _ => return,
        }
        retain(output, self);
    }

    /// Preserve the three distinct multiline title languages and their priorities.
    fn title(mut self, scalar: char, position: Range<usize>, output: &mut Vec<Self>) {
        match (self.phase, scalar) {
            (Phase::Double, '\\') => self.phase = Phase::DoubleEscape,
            (Phase::DoubleEscape, '"') => {
                let mut escaped = self.clone();
                escaped.phase = Phase::Double;
                retain(output, escaped);
                self.close_title(position.end);
            }
            (Phase::Double, '"')
            | (Phase::Single | Phase::SingleLine, '\'')
            | (Phase::Parenthesized, ')') => self.close_title(position.end),
            (Phase::DoubleEscape, '\\') => {}
            (Phase::Double | Phase::DoubleEscape, _) => self.phase = Phase::Double,
            (Phase::Single, '\n') => self.phase = Phase::SingleLine,
            (Phase::SingleLine, '\n') | (Phase::Parenthesized, '(') => return,
            (Phase::SingleLine, _) => self.phase = Phase::Single,
            _ => {}
        }
        retain(output, self);
    }

    /// Close a title before the shared trailing-space grammar.
    fn close_title(&mut self, end: usize) {
        if let Some(title) = &mut self.matched.title {
            title.end = end;
        }
        self.phase = Phase::Spaces;
    }

    /// Retain greedy trailing newlines and an earlier successful prefix.
    fn ending(mut self, scalar: char, position: Range<usize>, output: &mut Vec<Self>) {
        if scalar == '\n' {
            self.phase = Phase::Newlines;
            self.matched.consumed = position.end;
            let mut accepted = self.clone();
            accepted.phase = Phase::Accepted;
            retain(output, self);
            retain(output, accepted);
        } else if scalar == ' ' && self.phase != Phase::Newlines {
            self.phase = Phase::Spaces;
            retain(output, self);
        }
    }

    /// Resolve input-end alternatives only when the caller owns the final boundary.
    fn finish(&self, end: usize) -> Option<Definition> {
        match self.phase {
            Phase::Destination | Phase::Suffix | Phase::Spaces | Phase::Newlines => {}
            Phase::Accepted => return Some(self.matched.clone()),
            _ => return None,
        }
        let mut matched = self.matched.clone();
        if self.phase == Phase::Destination {
            matched.destination.end = end;
        }
        matched.consumed = end;
        Some(matched)
    }
}

/// Keep the first alternative per grammar position and discard paths behind success.
fn retain(output: &mut Vec<Candidate>, candidate: Candidate) {
    if output
        .iter()
        .any(|item| item.phase == Phase::Accepted || item.phase == candidate.phase)
    {
        return;
    }
    output.push(candidate);
}

/// ECMAScript dot's line terminators, not the broader whitespace class.
fn line_end(scalar: char) -> bool {
    matches!(scalar, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}
