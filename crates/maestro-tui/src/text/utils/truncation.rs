//! Truncation to a visible width with an independently framed ellipsis.

use super::measure::{TAB_CELLS, visible_width};
use super::parsed::{Grapheme, Parsed};
use super::render::{Emission, RESET};

/// How [`truncate_to_width`] marks and fills a shortened text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TruncateOptions<'a> {
    /// Text appended after the kept prefix when something was cut.
    pub ellipsis: &'a str,
    /// Whether the result is padded with spaces up to the full width, measured on the
    /// finished result.
    pub pad: bool,
}

impl Default for TruncateOptions<'_> {
    fn default() -> Self {
        Self {
            ellipsis: "...",
            pad: false,
        }
    }
}

/// The graphemes of a parsed text, segmented once, and how much of it fits a width.
struct Scan<'p, 'a> {
    /// Text and escapes the graphemes belong to.
    parsed: &'p Parsed<'a>,
    /// Graphemes in order, ending with the first one that takes the total past the limit.
    graphemes: Vec<Grapheme>,
    /// Cells of those graphemes.
    cells: usize,
    /// Whether the whole text measures at most the limit.
    fits: bool,
}

impl<'p, 'a> Scan<'p, 'a> {
    /// Segments `parsed`, stopping once more than `limit` cells were seen.
    fn new(parsed: &'p Parsed<'a>, limit: usize) -> Self {
        let mut graphemes = Vec::new();
        let mut cells = 0;
        for grapheme in parsed.graphemes(0..parsed.visible.len(), TAB_CELLS) {
            cells += grapheme.cells;
            graphemes.push(grapheme);
            if cells > limit {
                break;
            }
        }
        Self {
            parsed,
            graphemes,
            cells,
            fits: cells <= limit,
        }
    }

    /// How many leading graphemes together measure at most `budget`.
    fn prefix_within(&self, budget: usize) -> usize {
        let mut used = 0;
        self.graphemes
            .iter()
            .take_while(|grapheme| {
                used += grapheme.cells;
                used <= budget
            })
            .count()
    }

    /// Emits the first `count` graphemes with every escape anchored at or inside them.
    fn emit(&self, count: usize) -> Emission<'p, 'a> {
        let mut emission = Emission::new(self.parsed);
        for grapheme in &self.graphemes[..count] {
            emission.grapheme(grapheme);
        }
        emission
    }

    /// The first `count` graphemes, with a hyperlink still open at their end closed.
    fn render_prefix(&self, count: usize) -> String {
        self.emit(count).finish(false)
    }

    /// The whole text, or only its metadata escapes when it has no visible content.
    ///
    /// The scan must fit its limit, so that it holds every grapheme of the text.
    fn render_whole(&self) -> String {
        let mut emission = self.emit(self.graphemes.len());
        let events = self.parsed.events.len();
        match self.graphemes.last() {
            Some(last) => emission.events(last.events.end..events),
            None => emission.opaque_events(0..events),
        }
        emission.finish(false)
    }
}

/// Fills a finished result with spaces up to `max_width` as measured on the result itself.
fn pad_measured(text: String, max_width: usize) -> String {
    let fill = max_width.saturating_sub(visible_width(&text));
    text + &" ".repeat(fill)
}

/// Joins prefix and ellipsis with the resets that isolate them; empty when both are empty.
fn frame(prefix: &str, ellipsis: &str) -> String {
    match (prefix.is_empty(), ellipsis.is_empty()) {
        (true, true) => String::new(),
        (_, true) => format!("{prefix}{RESET}"),
        _ => format!("{prefix}{RESET}{ellipsis}{RESET}"),
    }
}

/// Drops graphemes from the end of the `kept` leading ones of `scan` until the prefix framed
/// with `ellipsis` measures at most `max_width`, or none remain.
///
/// Prefix and ellipsis can join into one grapheme, such as a mark that widens its base, so
/// each candidate is measured as it will be emitted.
fn fit_prefix(scan: &Scan<'_, '_>, mut kept: usize, ellipsis: &str, max_width: usize) -> String {
    let mut candidate = frame(&scan.render_prefix(kept), ellipsis);
    while visible_width(&candidate) > max_width && kept > 0 {
        kept -= 1;
        candidate = frame(&scan.render_prefix(kept), ellipsis);
    }
    candidate
}

/// The text cut to `max_width` columns, before any padding.
fn shorten(text: &str, max_width: usize, ellipsis: &str) -> String {
    let parsed = Parsed::parse(text);
    let scan = Scan::new(&parsed, max_width);
    if scan.fits {
        return scan.render_whole();
    }
    let parsed_ellipsis = Parsed::parse(ellipsis);
    let marker = Scan::new(&parsed_ellipsis, max_width);
    if marker.cells >= max_width {
        return match marker.prefix_within(max_width) {
            0 => String::new(),
            clipped => frame("", &marker.render_prefix(clipped)),
        };
    }
    let kept = scan.prefix_within(max_width - marker.cells);
    fit_prefix(&scan, kept, &marker.render_whole(), max_width)
}

/// Returns `text` without the ellipsis when it measures at most `max_width`, and otherwise a
/// prefix of whole graphemes followed by the ellipsis, each followed by a full reset.
///
/// The result measures at most `max_width`. An ellipsis as wide as `max_width` or wider is
/// returned alone, cut to the whole graphemes that fit. A `max_width` of zero gives an empty
/// string.
#[must_use]
pub fn truncate_to_width(text: &str, max_width: usize, options: TruncateOptions<'_>) -> String {
    if max_width == 0 {
        return String::new();
    }
    let shortened = shorten(text, max_width, options.ellipsis);
    if options.pad {
        pad_measured(shortened, max_width)
    } else {
        shortened
    }
}
