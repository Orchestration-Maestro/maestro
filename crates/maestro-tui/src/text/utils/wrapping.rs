//! Greedy word wrapping that writes text and escapes in source order.

use std::ops::Range;

use super::is_whitespace_scalar;
use super::measure::TAB_CELLS;
use super::parsed::{Grapheme, Parsed};
use super::render::{Emission, trim_end};

/// Breaks `text` into unpadded lines at ASCII spaces, and between whole graphemes inside a
/// word wider than `width`.
///
/// Escapes are read within a literal line, never across a line feed, and stay where they were
/// written: inside a word wider than `width`, an escape before the grapheme that starts a new
/// row ends the row it follows, and one before a whole word that moves to a new row starts
/// that row. A row that has text begins with the style in effect where it starts and closes
/// the hyperlink still open at its end. A non-whitespace grapheme wider than `width` is kept
/// whole.
#[must_use]
pub fn wrap_text_with_ansi(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let parsed = Parsed::parse_lines(text);
    parsed
        .lines()
        .into_iter()
        .flat_map(|span| wrap_line(&parsed, span, width))
        .collect()
}

/// A run of graphemes between ASCII spaces, or a run of spaces, with the escapes written before
/// its graphemes. The last token also carries the escapes after the last grapheme.
struct Token {
    /// Grapheme indices of the run.
    graphemes: Range<usize>,
    /// Index after the last escape the run carries.
    events_end: usize,
    /// Cells the run occupies.
    cells: usize,
    /// Whether the run holds only whitespace and no escape.
    whitespace: bool,
}

/// A wrapped row before rows that show nothing are removed.
struct Candidate {
    /// The row as written.
    text: String,
    /// Byte position after the inherited style that opens the row.
    prefix: usize,
    /// Source escapes the row holds.
    events: Range<usize>,
}

/// The row being filled.
struct Row<'p, 'a> {
    /// Text of the row so far.
    emission: Emission<'p, 'a>,
    /// Byte position after the inherited style that opens the row.
    prefix: usize,
    /// Index of the first source escape the row holds.
    first_event: usize,
}

/// The rows one literal line wraps into, filled in source order.
struct Wrap<'p, 'a> {
    /// Source of the text and escapes.
    parsed: &'p Parsed<'a>,
    /// Cells a row may hold.
    width: usize,
    /// Rows finished so far.
    rows: Vec<Candidate>,
    /// Row being filled; `None` until a source escape or grapheme is written to it, so no row
    /// holds only the style it inherits.
    row: Option<Row<'p, 'a>>,
    /// Cells of the row being filled.
    cells: usize,
    /// Style state of the source read so far, kept across the closes written at row ends.
    state: usize,
    /// Index of the next source escape to write.
    cursor: usize,
}

impl<'p, 'a> Row<'p, 'a> {
    /// Opens a row in `state`, holding source escapes from `first_event` on.
    fn open(parsed: &'p Parsed<'a>, state: usize, first_event: usize) -> Self {
        let mut emission = Emission::new(parsed);
        let prefix = emission.inherit(state);
        Self {
            emission,
            prefix,
            first_event,
        }
    }
}

impl<'p, 'a> Wrap<'p, 'a> {
    /// The row being filled, opened when there is none.
    fn row(&mut self) -> &mut Emission<'p, 'a> {
        let (parsed, state, cursor) = (self.parsed, self.state, self.cursor);
        &mut self
            .row
            .get_or_insert_with(|| Row::open(parsed, state, cursor))
            .emission
    }

    /// Writes source escapes to the row.
    fn events(&mut self, range: Range<usize>) {
        let end = range.end;
        self.row().events(range);
        self.cursor = end;
    }

    /// Writes a grapheme's text and the escapes inside it to the row.
    fn content(&mut self, grapheme: &Grapheme) {
        self.row().content(grapheme);
        self.cursor = grapheme.events.end;
        self.cells += grapheme.cells;
    }

    /// Writes a whole token to the row.
    fn append(&mut self, graphemes: &[Grapheme], token: &Token) {
        for grapheme in &graphemes[token.graphemes.clone()] {
            self.events(self.parsed.leading_events(grapheme));
            self.content(grapheme);
        }
        self.events(self.cursor..token.events_end);
    }

    /// Places a token of an overflowing line: a word wider than the width breaks between
    /// graphemes, a token that does not fit starts a new row, and whitespace never opens one.
    fn token(&mut self, graphemes: &[Grapheme], token: &Token) {
        if token.cells > self.width && !token.whitespace {
            self.flush(false);
            self.long_token(graphemes, token);
            return;
        }
        if self.cells > 0 && self.cells + token.cells > self.width {
            if token.whitespace || self.paints_nothing(graphemes, token) {
                self.flush_blank(graphemes, token);
                return;
            }
            self.flush(true);
        }
        self.append(graphemes, token);
    }

    /// Whether every grapheme of a token is a blank its style leaves unpainted.
    fn paints_nothing(&self, graphemes: &[Grapheme], token: &Token) -> bool {
        graphemes[token.graphemes.clone()].iter().all(|grapheme| {
            is_whitespace(self.parsed, grapheme) && !self.parsed.shows_blank(grapheme)
        })
    }

    /// Ends the row at a blank token, which is dropped instead of starting the next row, also
    /// when escapes are attached to it. Its metadata escapes and the escapes after the
    /// line's last grapheme end the row; its style escapes only carry over to the next row.
    fn flush_blank(&mut self, graphemes: &[Grapheme], token: &Token) {
        let start = graphemes[token.graphemes.start].events.start;
        let trailing = graphemes[token.graphemes.end - 1].events.end;
        if let Some(row) = self.row.as_mut() {
            row.emission.trim_end();
            row.emission.opaque_events(start..trailing);
            row.emission.events(trailing..token.events_end);
        }
        self.cursor = token.events_end;
        self.flush(false);
        if token.events_end > start {
            self.state = self.parsed.events[token.events_end - 1].state;
        }
    }

    /// Breaks a word between whole graphemes. The escapes before a grapheme are written before
    /// its width is tested, so they end the row when the grapheme starts the next one.
    fn long_token(&mut self, graphemes: &[Grapheme], token: &Token) {
        for grapheme in &graphemes[token.graphemes.clone()] {
            self.events(self.parsed.leading_events(grapheme));
            if grapheme.cells > 0 && self.cells > 0 && self.cells + grapheme.cells > self.width {
                self.flush(false);
            }
            self.content(grapheme);
        }
        self.events(self.cursor..token.events_end);
    }

    /// Ends the row being filled, closing underline and the hyperlink, and trims its trailing
    /// whitespace; `trim` trims it before the closes as well.
    fn flush(&mut self, trim: bool) {
        if let Some(mut row) = self.row.take() {
            if trim {
                row.emission.trim_end();
            }
            self.state = row.emission.state();
            let mut text = row.emission.finish(true);
            trim_end(&mut text);
            self.rows.push(Candidate {
                text,
                prefix: row.prefix,
                events: row.first_event..self.cursor,
            });
        }
        self.cells = 0;
    }

    /// Ends the line with its last row, closing only the hyperlink.
    fn finish(mut self, trim: bool) -> Vec<Candidate> {
        if let Some(mut row) = self.row.take() {
            if trim {
                row.emission.trim_end();
            }
            self.rows.push(Candidate {
                text: row.emission.finish(false),
                prefix: row.prefix,
                events: row.first_event..self.cursor,
            });
        }
        self.rows
    }
}

/// Wraps one literal line.
fn wrap_line(parsed: &Parsed<'_>, span: Range<usize>, width: usize) -> Vec<String> {
    let events = parsed.events_in(&span);
    let graphemes: Vec<Grapheme> = parsed.graphemes(span, TAB_CELLS).collect();
    let Some(first) = graphemes.first() else {
        return vec![metadata(parsed, events)];
    };
    let trailing = graphemes
        .last()
        .map_or(events.start, |last| last.events.end)..events.end;
    let fits = graphemes
        .iter()
        .map(|grapheme| grapheme.cells)
        .sum::<usize>()
        <= width;
    let mut wrap = Wrap {
        parsed,
        width,
        rows: Vec::new(),
        row: None,
        cells: 0,
        state: first.state,
        cursor: events.start,
    };
    for token in tokens(parsed, &graphemes, events.end) {
        if fits {
            wrap.append(&graphemes, &token);
        } else {
            wrap.token(&graphemes, &token);
        }
    }
    let rows = wrap.finish(!fits);
    if fits {
        rows.into_iter().map(|row| row.text).collect()
    } else {
        select(parsed, rows, trailing)
    }
}

/// Splits `graphemes` into runs at ASCII spaces; the last run takes the escapes up to
/// `events_end`.
fn tokens(parsed: &Parsed<'_>, graphemes: &[Grapheme], events_end: usize) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut previous_space = false;
    for (index, grapheme) in graphemes.iter().enumerate() {
        let space = parsed.text(grapheme) == " ";
        let whitespace = grapheme.events.is_empty() && is_whitespace(parsed, grapheme);
        match tokens.last_mut() {
            Some(token) if space == previous_space => {
                token.graphemes.end = index + 1;
                token.events_end = grapheme.events.end;
                token.cells += grapheme.cells;
                token.whitespace &= whitespace;
            }
            _ => tokens.push(Token {
                graphemes: index..index + 1,
                events_end: grapheme.events.end,
                cells: grapheme.cells,
                whitespace,
            }),
        }
        previous_space = space;
    }
    if let Some(last) = tokens.last_mut() {
        last.whitespace &= last.events_end == events_end;
        last.events_end = events_end;
    }
    tokens
}

/// Removes the rows of an overflowing line that show nothing.
///
/// Each metadata escape is written once, by the row that holds it. The metadata escapes of a
/// removed row move to the next row kept, after its inherited style, or to the end of the last
/// row kept, followed by the escapes after the line's last grapheme that removed rows held. A
/// line left without rows keeps one holding only its metadata escapes.
fn select(parsed: &Parsed<'_>, candidates: Vec<Candidate>, trailing: Range<usize>) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    let mut held = String::new();
    let mut lost = Vec::new();
    for Candidate {
        mut text,
        prefix,
        events,
    } in candidates
    {
        if shows(&text) {
            text.insert_str(prefix, &std::mem::take(&mut held));
            rows.push(text);
        } else {
            let split = events.end.min(trailing.start).max(events.start);
            held.push_str(&metadata(parsed, events.start..split));
            lost.push(split..events.end);
        }
    }
    if let Some(last) = rows.last_mut() {
        let mut tail = Emission::new(parsed);
        for range in lost {
            tail.events(range);
        }
        last.push_str(&held);
        last.push_str(&tail.into_string());
    } else {
        rows.push(
            lost.into_iter()
                .fold(held, |row, range| row + &metadata(parsed, range)),
        );
    }
    rows
}

/// Whether a wrapped row shows anything: a grapheme that is not whitespace, a blank its style
/// paints, or an inverse or background style left open that paints the cells after the row.
fn shows(row: &str) -> bool {
    let parsed = Parsed::parse(row);
    let end = parsed.events.last().map_or(0, |event| event.state);
    parsed
        .graphemes(0..parsed.visible.len(), TAB_CELLS)
        .any(|grapheme| !is_whitespace(&parsed, &grapheme) || parsed.shows_blank(&grapheme))
        || parsed.states[end].paints_after_row()
}

/// The metadata escapes of a range of source escapes, without style escapes.
fn metadata(parsed: &Parsed<'_>, range: Range<usize>) -> String {
    let mut emission = Emission::new(parsed);
    emission.opaque_events(range);
    emission.into_string()
}

/// Whether `grapheme` is whitespace by its text.
fn is_whitespace(parsed: &Parsed<'_>, grapheme: &Grapheme) -> bool {
    parsed.text(grapheme).chars().all(is_whitespace_scalar)
}
