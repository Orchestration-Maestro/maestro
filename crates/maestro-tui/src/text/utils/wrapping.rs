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

/// Wraps `text` into lines of at most `width` columns, keeping escapes live across breaks.
///
/// A single grapheme wider than `width` is kept alone on its line instead of being clipped.
#[must_use]
pub fn wrap_text_with_ansi(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let parsed = Parsed::parse(text);
    parsed
        .lines()
        .into_iter()
        .flat_map(|span| wrap_line(&parsed, &span, width))
        .collect()
}

/// A wrapped line: the graphemes it was filled with and the part that stays visible.
struct Placed {
    /// Graphemes added to the line, trailing whitespace included.
    added: Range<usize>,
    /// The added graphemes without trailing whitespace.
    kept: Range<usize>,
}

/// Wraps one literal line.
fn wrap_line(parsed: &Parsed<'_>, span: &Range<usize>, width: usize) -> Vec<String> {
    let graphemes: Vec<Grapheme> = parsed.graphemes(span.clone(), TAB_CELLS).collect();
    let events = parsed.events_in(span);
    let trailing_start = graphemes
        .last()
        .map_or(events.start, |last| last.events.end);
    let total: usize = graphemes.iter().map(|grapheme| grapheme.cells).sum();
    let fits = total <= width;
    let added = if fits {
        std::iter::once(0..graphemes.len()).collect()
    } else {
        line_ranges(parsed, &graphemes, width)
    };
    let placed: Vec<Placed> = added
        .into_iter()
        .map(|added| Placed {
            kept: if fits {
                added.clone()
            } else {
                trim_end(parsed, &graphemes, added.clone())
            },
            added,
        })
        .filter(|line| !line.kept.is_empty())
        .collect();
    if placed.is_empty() {
        let mut empty = Emission::new(parsed);
        empty.opaque_events(events);
        return vec![empty.into_string()];
    }
    let event_start = |index: usize| {
        graphemes
            .get(index)
            .map_or(trailing_start, |g| g.events.start)
    };
    let last = placed.len() - 1;
    placed
        .iter()
        .enumerate()
        .map(|(position, line)| {
            let mut emission = Emission::new(parsed);
            emission.inherit(graphemes[line.kept.start].state);
            for grapheme in &graphemes[line.kept.clone()] {
                emission.grapheme(grapheme);
            }
            emission.events(event_start(line.kept.end)..event_start(line.added.end));
            let dropped_end = placed
                .get(position + 1)
                .map_or(trailing_start, |next| event_start(next.kept.start));
            emission.opaque_events(event_start(line.added.end)..dropped_end);
            if position == last {
                emission.events(trailing_start..events.end);
            }
            emission.finish(position != last)
        })
        .collect()
}

/// Grapheme ranges of the lines a too-wide logical line breaks into.
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
        let whitespace = text.chars().all(is_whitespace_scalar);
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

/// Drops whitespace graphemes from the end of a line.
fn trim_end(parsed: &Parsed<'_>, graphemes: &[Grapheme], range: Range<usize>) -> Range<usize> {
    let kept = graphemes[range.clone()]
        .iter()
        .rposition(|grapheme| !parsed.text(grapheme).chars().all(is_whitespace_scalar))
        .map_or(0, |last| last + 1);
    range.start..range.start + kept
}
