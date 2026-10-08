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

/// The contiguous prefix of a text that fits a budget, and whether the whole text fits.
struct Scan {
    /// Graphemes kept, in order.
    kept: Vec<Grapheme>,
    /// Whether the whole text measures at most the width.
    fits: bool,
}

impl Scan {
    /// Keeps graphemes while they fit `budget`; stops measuring once `max_width` is passed.
    fn new(parsed: &Parsed<'_>, budget: usize, max_width: usize) -> Self {
        let mut kept = Vec::new();
        let mut kept_cells = 0;
        let mut seen = 0;
        let mut keeping = true;
        for grapheme in parsed.graphemes(0..parsed.visible.len(), TAB_CELLS) {
            let cells = grapheme.cells;
            if keeping && kept_cells + cells <= budget {
                kept_cells += cells;
                kept.push(grapheme);
            } else {
                keeping = false;
            }
            seen += cells;
            if seen > max_width {
                return Self { kept, fits: false };
            }
        }
        Self { kept, fits: true }
    }
}

/// Fills a finished result with spaces up to `max_width` as measured on the result itself.
fn pad_measured(text: String, max_width: usize) -> String {
    let fill = max_width.saturating_sub(visible_width(&text));
    text + &" ".repeat(fill)
}

/// Emits a whole text, or only its metadata when it has no visible content.
fn render_all(parsed: &Parsed<'_>) -> String {
    let mut emission = Emission::new(parsed);
    let mut trailing = 0;
    for grapheme in parsed.graphemes(0..parsed.visible.len(), TAB_CELLS) {
        emission.grapheme(&grapheme);
        trailing = grapheme.events.end;
    }
    if parsed.visible.is_empty() {
        emission.opaque_events(0..parsed.events.len());
    } else {
        emission.events(trailing..parsed.events.len());
    }
    emission.finish(false)
}

/// Emits the kept graphemes and closes their hyperlink.
fn render_kept(parsed: &Parsed<'_>, kept: &[Grapheme]) -> String {
    let mut emission = Emission::new(parsed);
    for grapheme in kept {
        emission.grapheme(grapheme);
    }
    emission.finish(false)
}

/// Joins prefix and ellipsis with the resets that isolate them; empty when both are empty.
fn frame(prefix: &str, ellipsis: &str) -> String {
    match (prefix.is_empty(), ellipsis.is_empty()) {
        (true, true) => String::new(),
        (_, true) => format!("{prefix}{RESET}"),
        _ => format!("{prefix}{RESET}{ellipsis}{RESET}"),
    }
}

/// Drops graphemes from the end of `kept` until the prefix framed with `ellipsis` measures at
/// most `max_width`, or none remain.
///
/// Prefix and ellipsis can join into one grapheme, such as a mark that widens its base, so
/// each candidate is measured as it will be emitted.
fn fit_prefix(
    parsed: &Parsed<'_>,
    mut kept: Vec<Grapheme>,
    ellipsis: &str,
    max_width: usize,
) -> String {
    let mut candidate = frame(&render_kept(parsed, &kept), ellipsis);
    while visible_width(&candidate) > max_width && kept.pop().is_some() {
        candidate = frame(&render_kept(parsed, &kept), ellipsis);
    }
    candidate
}

/// The text cut to `max_width` columns, before any padding.
fn shorten(text: &str, max_width: usize, ellipsis: &str) -> String {
    let ellipsis_cells = visible_width(ellipsis);
    let parsed = Parsed::parse(text);
    let scan = Scan::new(&parsed, max_width.saturating_sub(ellipsis_cells), max_width);
    if scan.fits {
        return render_all(&parsed);
    }
    let ellipsis = Parsed::parse(ellipsis);
    if ellipsis_cells >= max_width {
        let clipped = Scan::new(&ellipsis, max_width, max_width).kept;
        return if clipped.is_empty() {
            String::new()
        } else {
            frame("", &render_kept(&ellipsis, &clipped))
        };
    }
    fit_prefix(&parsed, scan.kept, &render_all(&ellipsis), max_width)
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
