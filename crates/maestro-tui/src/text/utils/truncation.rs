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
    /// Cells of the kept graphemes.
    kept_cells: usize,
    /// Cells of the whole text when it fits the width, otherwise `None`.
    total: Option<usize>,
}

impl Scan {
    /// Keeps graphemes while they fit `budget`; stops measuring once `max_width` is passed.
    fn new(parsed: &Parsed<'_>, budget: usize, max_width: usize) -> Self {
        let mut scan = Self {
            kept: Vec::new(),
            kept_cells: 0,
            total: None,
        };
        let mut seen = 0;
        let mut keeping = true;
        for grapheme in parsed.graphemes(0..parsed.visible.len(), TAB_CELLS) {
            if keeping && scan.kept_cells + grapheme.cells <= budget {
                scan.kept_cells += grapheme.cells;
                scan.kept.push(grapheme.clone());
            } else {
                keeping = false;
            }
            seen += grapheme.cells;
            if seen > max_width {
                return scan;
            }
        }
        scan.total = Some(seen);
        scan
    }
}

/// Spaces that fill `cells` columns when padding is requested.
fn padding(pad: bool, cells: usize) -> String {
    if pad {
        " ".repeat(cells)
    } else {
        String::new()
    }
}

/// Fills a finished result with spaces up to `max_width` as measured on the result itself.
///
/// Prefix and ellipsis can join into one cluster, such as a flag, that is narrower than
/// the sum of their widths.
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

/// Shortens `text` to at most `max_width` columns, appending the ellipsis when it cuts.
#[must_use]
pub fn truncate_to_width(text: &str, max_width: usize, options: TruncateOptions<'_>) -> String {
    if max_width == 0 {
        return String::new();
    }
    if text.is_empty() {
        return padding(options.pad, max_width);
    }
    let ellipsis_cells = visible_width(options.ellipsis);
    let parsed = Parsed::parse(text);
    let scan = Scan::new(&parsed, max_width.saturating_sub(ellipsis_cells), max_width);
    if let Some(total) = scan.total {
        return render_all(&parsed) + &padding(options.pad, max_width - total);
    }
    let ellipsis = Parsed::parse(options.ellipsis);
    let framed = if ellipsis_cells >= max_width {
        let clipped = Scan::new(&ellipsis, max_width, max_width);
        if clipped.kept.is_empty() {
            return padding(options.pad, max_width);
        }
        frame("", &render_kept(&ellipsis, &clipped.kept))
    } else {
        frame(&render_kept(&parsed, &scan.kept), &render_all(&ellipsis))
    };
    if options.pad {
        pad_measured(framed, max_width)
    } else {
        framed
    }
}
