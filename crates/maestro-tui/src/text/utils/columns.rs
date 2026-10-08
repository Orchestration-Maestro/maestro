//! Selection of whole graphemes by terminal column.

use std::ops::Range;

use super::parsed::{Grapheme, Parsed};
use super::render::Emission;

/// Text selected by columns and the cells it actually occupies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnSlice {
    /// The selected text with the escapes that govern it.
    pub text: String,
    /// Terminal cells the selected graphemes occupy.
    pub width: usize,
}

/// The text before an overlay and the text after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtractedSegments {
    /// Text that starts left of the overlay.
    pub before: String,
    /// Cells of [`before`](Self::before).
    pub before_width: usize,
    /// Text that starts inside the requested range after the overlay.
    pub after: String,
    /// Cells of [`after`](Self::after).
    pub after_width: usize,
}

/// Graphemes selected for one output, the cells they use and the escapes that follow them.
#[derive(Default)]
struct Run {
    /// Selected graphemes in order.
    graphemes: Vec<Grapheme>,
    /// Cells of the selected graphemes.
    cells: usize,
    /// Escapes positioned after the last selected grapheme but still inside the range.
    trailing: Range<usize>,
}

impl Run {
    /// Adds a grapheme to the selection.
    fn push(&mut self, grapheme: Grapheme) {
        self.cells += grapheme.cells;
        self.graphemes.push(grapheme);
    }
}

/// Walks the graphemes of a line with the columns each spans until `visit` returns `true`.
///
/// Returns the column reached and the escapes after the last grapheme when the walk
/// consumed the whole line.
fn walk(
    parsed: &Parsed<'_>,
    mut visit: impl FnMut(Range<usize>, Grapheme) -> bool,
) -> (usize, Option<Range<usize>>) {
    let mut col = 0usize;
    let mut trailing = 0;
    for grapheme in parsed.graphemes(0..parsed.visible.len(), 0) {
        let next = col.saturating_add(grapheme.cells);
        trailing = grapheme.events.end;
        let stop = visit(col..next, grapheme);
        col = next;
        if stop {
            return (col, None);
        }
    }
    (col, Some(trailing..parsed.events.len()))
}

/// Selects the graphemes that start in `start..end`, optionally only those that also end there.
fn select(parsed: &Parsed<'_>, start: usize, end: usize, strict: bool) -> Run {
    let mut run = Run::default();
    let (col, tail) = walk(parsed, |cols, grapheme| {
        if cols.start >= start && cols.start < end {
            if !strict || cols.end <= end {
                run.push(grapheme);
            } else {
                run.trailing = parsed.leading_events(&grapheme);
            }
        }
        cols.end >= end
    });
    if let Some(tail) = tail.filter(|_| col >= start && col < end) {
        run.trailing = tail;
    }
    run
}

/// Emits a selection, carrying every earlier escape in front of its first grapheme.
///
/// A selection without graphemes emits only the metadata escapes of its range.
fn render_run(parsed: &Parsed<'_>, run: &Run) -> String {
    let mut emission = Emission::new(parsed);
    if let Some(first) = run.graphemes.first() {
        emission.events(0..first.events.start);
        for grapheme in &run.graphemes {
            emission.grapheme(grapheme);
        }
        emission.events(run.trailing.clone());
    } else {
        emission.opaque_events(run.trailing.clone());
    }
    emission.finish(false)
}

/// The text occupying `length` columns from `start_col`, with its width in cells.
#[must_use]
pub fn slice_with_width(line: &str, start_col: usize, length: usize, strict: bool) -> ColumnSlice {
    if length == 0 {
        return ColumnSlice {
            text: String::new(),
            width: 0,
        };
    }
    let parsed = Parsed::parse(line);
    let run = select(&parsed, start_col, start_col.saturating_add(length), strict);
    ColumnSlice {
        text: render_run(&parsed, &run),
        width: run.cells,
    }
}

/// The text occupying `length` columns from `start_col`.
#[must_use]
pub fn slice_by_column(line: &str, start_col: usize, length: usize, strict: bool) -> String {
    slice_with_width(line, start_col, length, strict).text
}

/// Splits a line into the text before `before_end` and the text in `after_start..+after_len`.
///
/// The text after the overlay starts from the style in effect at its first grapheme.
///
/// Metadata escapes stay in place in either part: between their graphemes, in front of
/// the first grapheme after, and at the end of the text before when the line ends inside
/// it. The part before wins where the two overlap; metadata under the overlay is dropped.
#[must_use]
pub fn extract_segments(
    line: &str,
    before_end: usize,
    after_start: usize,
    after_len: usize,
    strict_after: bool,
) -> ExtractedSegments {
    let parsed = Parsed::parse(line);
    let after_end = after_start.saturating_add(after_len);
    let mut before = Run::default();
    let mut after = Run::default();
    let (col, tail) = walk(&parsed, |cols, grapheme| {
        if cols.start < before_end {
            before.push(grapheme);
        } else if cols.start >= after_start && cols.start < after_end {
            if !strict_after || cols.end <= after_end {
                after.push(grapheme);
            } else {
                after.trailing = parsed.leading_events(&grapheme);
            }
        }
        if after_len == 0 {
            cols.end >= before_end
        } else {
            cols.end >= after_end
        }
    });
    if let Some(tail) = tail {
        if col < before_end {
            before.trailing = tail;
        } else if col >= after_start && col < after_end {
            after.trailing = tail;
        }
    }
    ExtractedSegments {
        before: render_before(&parsed, &before),
        before_width: before.cells,
        after: render_after(&parsed, &after),
        after_width: after.cells,
    }
}

/// Emits the text before an overlay: its graphemes and the metadata that ends the region.
fn render_before(parsed: &Parsed<'_>, run: &Run) -> String {
    let mut emission = Emission::new(parsed);
    for grapheme in &run.graphemes {
        emission.grapheme(grapheme);
    }
    emission.opaque_events(run.trailing.clone());
    emission.finish(false)
}

/// Emits the text after an overlay, starting from the style in effect at its first grapheme.
fn render_after(parsed: &Parsed<'_>, run: &Run) -> String {
    let Some((first, rest)) = run.graphemes.split_first() else {
        return render_run(parsed, run);
    };
    let leading = parsed.leading_events(first);
    let state = leading
        .end
        .checked_sub(1)
        .filter(|_| !leading.is_empty())
        .map_or(first.state, |last| parsed.events[last].state);
    let mut emission = Emission::new(parsed);
    emission.inherit(state);
    emission.opaque_events(leading);
    emission.content(first);
    for grapheme in rest {
        emission.grapheme(grapheme);
    }
    emission.events(run.trailing.clone());
    emission.finish(false)
}
