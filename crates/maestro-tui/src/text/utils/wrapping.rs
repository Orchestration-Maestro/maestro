//! Greedy word wrapping over grapheme ranges.

use std::ops::Range;

use super::is_whitespace_scalar;
use super::measure::TAB_CELLS;
use super::parsed::{Grapheme, Parsed};
use super::render::Emission;

/// A run of graphemes between ASCII spaces, or a run of spaces.
struct Token {
    /// Grapheme indices of the run.
    range: Range<usize>,
    /// Cells the run occupies.
    cells: usize,
    /// Whether every scalar of the run is whitespace.
    whitespace: bool,
}

/// Grapheme indices and cells of the line being filled.
struct Fill {
    /// Grapheme indices on the line.
    range: Range<usize>,
    /// Cells used so far.
    cells: usize,
}

/// Breaks `text` into unpadded lines at ASCII spaces, and between whole graphemes inside a
/// word wider than `width`.
///
/// Escapes are read within a literal line, never across a line feed. A line that has text
/// begins with the style in effect at its first grapheme and closes the hyperlink still open
/// at its end. A non-whitespace grapheme wider than `width` is kept whole.
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

/// A wrapped line: the graphemes it was filled with and the part that stays visible.
struct Placed {
    /// Graphemes added to the line, trailing whitespace included.
    added: Range<usize>,
    /// The added graphemes; the trailing whitespace is left out only when the literal line
    /// overflows the width.
    kept: Range<usize>,
}

/// Breaks the graphemes of one literal line into lines.
///
/// A literal line that fits the width stays whole, whitespace included. An overflowing one
/// is broken into ranges; trimming may leave an empty range whose terminal state still paints.
fn place_lines(
    parsed: &Parsed<'_>,
    graphemes: &[Grapheme],
    width: usize,
    styled_after_line: bool,
) -> Vec<Placed> {
    let total: usize = graphemes.iter().map(|grapheme| grapheme.cells).sum();
    let fits = total <= width;
    let added = if fits {
        std::iter::once(0..graphemes.len()).collect()
    } else {
        line_ranges(parsed, graphemes, width)
    };
    added
        .into_iter()
        .map(|added| {
            let end = if fits {
                added.end
            } else {
                trim_end(parsed, graphemes, &added, styled_after_line)
            };
            Placed {
                kept: added.start..end,
                added,
            }
        })
        .filter(|line| !line.added.is_empty())
        .collect()
}

/// Wraps one literal line.
fn wrap_line(parsed: &Parsed<'_>, span: Range<usize>, width: usize) -> Vec<String> {
    let events = parsed.events_in(&span);
    let graphemes: Vec<Grapheme> = parsed.graphemes(span, TAB_CELLS).collect();
    let trailing_start = graphemes
        .last()
        .map_or(events.start, |last| last.events.end);
    let overflows = graphemes
        .iter()
        .map(|grapheme| grapheme.cells)
        .sum::<usize>()
        > width;
    let styled_after_line = parsed.has_style_escape(trailing_start..events.end);
    let placed = place_lines(parsed, &graphemes, width, styled_after_line);
    render_lines(parsed, &graphemes, &placed, events, overflows)
}

/// Emits rows, committing transferred events only when their row survives.
fn render_lines(
    parsed: &Parsed<'_>,
    graphemes: &[Grapheme],
    placed: &[Placed],
    events: Range<usize>,
    overflows: bool,
) -> Vec<String> {
    let trailing_start = graphemes.last().map_or(events.start, |g| g.events.end);
    let event_start = |index: usize| {
        graphemes
            .get(index)
            .map_or(trailing_start, |g| g.events.start)
    };
    let mut consumed = events.start;
    let mut rows: Vec<String> = Vec::new();
    for (position, line) in placed.iter().enumerate() {
        let first = &graphemes[line.kept.start];
        let transferred = consumed > first.events.start;
        let mut emission = Emission::new(parsed);
        let prefix_end = emission.inherit(if transferred {
            parsed.state_after(first)
        } else {
            first.state
        });
        let start = event_start(line.kept.start);
        for (index, grapheme) in graphemes[line.kept.clone()].iter().enumerate() {
            if transferred && index == 0 {
                emission.content(grapheme);
            } else {
                emission.grapheme(grapheme);
            }
        }
        emission.events(consumed.max(event_start(line.kept.end))..event_start(line.added.end));
        let mut end = placed
            .get(position + 1)
            .map_or(trailing_start, |next| event_start(next.kept.start));
        emission.opaque_events(event_start(line.added.end)..end);
        if breaks_whitespace_run(parsed, graphemes, line.added.end) {
            let leading = parsed.leading_events(&graphemes[line.added.end]);
            end = leading.end;
            emission.events(leading);
        }
        let last = position + 1 == placed.len();
        if last {
            emission.events(trailing_start..events.end);
            end = events.end;
        }
        if shows(parsed, graphemes, line, overflows, emission.state()) {
            emission.insert_opaque_events(prefix_end, consumed.min(start)..start);
            rows.push(emission.finish(!last));
            consumed = end;
        }
    }
    let mut tail = Emission::new(parsed);
    tail.opaque_events(consumed..events.end);
    if let Some(last) = rows.last_mut() {
        last.push_str(&tail.into_string());
    } else {
        rows.push(tail.into_string());
    }
    rows
}

/// Whether a wrapped row shows anything: a grapheme that is not blank whitespace, a blank the
/// style paints, or a style left open that paints the cells after the row. Every row of a
/// literal line that fits the width shows, blank ones included.
fn shows(
    parsed: &Parsed<'_>,
    graphemes: &[Grapheme],
    line: &Placed,
    overflows: bool,
    state: usize,
) -> bool {
    !overflows
        || graphemes[line.kept.clone()]
            .iter()
            .any(|grapheme| !is_whitespace(parsed, grapheme) || parsed.shows_blank(grapheme))
        || parsed.states[state].paints_after_row()
}

/// Grapheme ranges of the lines a too-wide literal line breaks into.
fn line_ranges(parsed: &Parsed<'_>, graphemes: &[Grapheme], width: usize) -> Vec<Range<usize>> {
    let mut lines = Vec::new();
    let mut current: Option<Fill> = None;
    for token in tokens(parsed, graphemes) {
        if token.cells > width && !token.whitespace {
            lines.extend(current.take().map(|fill| fill.range));
            current = Some(break_word(graphemes, token.range, width, &mut lines));
            continue;
        }
        match current.as_mut() {
            Some(fill) if fill.cells > 0 && fill.cells + token.cells > width => {
                lines.push(fill.range.clone());
                current = (!token.whitespace).then_some(Fill {
                    range: token.range,
                    cells: token.cells,
                });
            }
            Some(fill) => {
                fill.range.end = token.range.end;
                fill.cells += token.cells;
            }
            None => {
                current = Some(Fill {
                    range: token.range,
                    cells: token.cells,
                });
            }
        }
    }
    lines.extend(current.map(|fill| fill.range));
    lines
}

/// Splits `graphemes` into runs at ASCII spaces.
fn tokens(parsed: &Parsed<'_>, graphemes: &[Grapheme]) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut previous_space = false;
    for (index, grapheme) in graphemes.iter().enumerate() {
        let text = parsed.text(grapheme);
        let space = text == " ";
        let whitespace = text.chars().all(is_whitespace_scalar) && !parsed.shows_blank(grapheme);
        match tokens.last_mut() {
            Some(token) if space == previous_space => {
                token.range.end = index + 1;
                token.cells += grapheme.cells;
                token.whitespace &= whitespace;
            }
            _ => tokens.push(Token {
                range: index..index + 1,
                cells: grapheme.cells,
                whitespace,
            }),
        }
        previous_space = space;
    }
    tokens
}

/// Breaks an over-wide word between whole graphemes; returns the unfinished last piece.
fn break_word(
    graphemes: &[Grapheme],
    word: Range<usize>,
    width: usize,
    lines: &mut Vec<Range<usize>>,
) -> Fill {
    let mut piece = Fill {
        range: word.start..word.start,
        cells: 0,
    };
    for (index, grapheme) in graphemes.iter().enumerate().take(word.end).skip(word.start) {
        if grapheme.cells > 0 && piece.cells > 0 && piece.cells + grapheme.cells > width {
            lines.push(piece.range.clone());
            piece = Fill {
                range: index..index,
                cells: 0,
            };
        }
        piece.range.end = index + 1;
        piece.cells += grapheme.cells;
    }
    piece
}

/// Whether `grapheme` is whitespace by its text.
fn is_whitespace(parsed: &Parsed<'_>, grapheme: &Grapheme) -> bool {
    parsed.text(grapheme).chars().all(is_whitespace_scalar)
}

/// Index after the last grapheme of `line` that a wrapped row keeps.
///
/// Trailing whitespace goes unless underline paints a blank row or a following escape protects it.
/// Background and inverse can still paint an empty range through the inherited terminal state.
fn trim_end(
    parsed: &Parsed<'_>,
    graphemes: &[Grapheme],
    line: &Range<usize>,
    styled_after_line: bool,
) -> usize {
    let blank = graphemes[line.clone()]
        .iter()
        .all(|grapheme| is_whitespace(parsed, grapheme));
    let mut end = line.end;
    while end > line.start && is_whitespace(parsed, &graphemes[end - 1]) {
        let styled = if end < graphemes.len() {
            breaks_whitespace_run(parsed, graphemes, end)
                && parsed.has_style_escape(parsed.leading_events(&graphemes[end]))
        } else {
            styled_after_line
        };
        if styled || (blank && parsed.states[parsed.state_after(&graphemes[end - 1])].underlined())
        {
            break;
        }
        end -= 1;
    }
    end
}

/// Whether the break before grapheme `index` splits a run of whitespace, so that the escapes
/// written before that grapheme stay on the row the run started on. They are what makes the
/// blanks of the run show, and with them the cells that follow the row.
fn breaks_whitespace_run(parsed: &Parsed<'_>, graphemes: &[Grapheme], index: usize) -> bool {
    let space = |grapheme: &Grapheme| parsed.text(grapheme) == " ";
    index > 0
        && index < graphemes.len()
        && is_whitespace(parsed, &graphemes[index - 1])
        && is_whitespace(parsed, &graphemes[index])
        && space(&graphemes[index - 1]) == space(&graphemes[index])
}
