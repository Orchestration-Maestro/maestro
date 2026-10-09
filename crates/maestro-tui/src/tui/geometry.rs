//! Overlay layout and style-isolated frame composition.

use crate::images::terminal_image::is_image_line;
use crate::text::expand_tabs;
use crate::text::utils::{extract_segments, slice_by_column, slice_with_width, visible_width};

use super::drawing::SEGMENT_RESET;
use super::{OverlayAnchor, OverlayMargin, OverlayMarginValue, OverlayOptions, SizeValue, TUI};

/// Resolved dimensions and screen-relative position.
struct Layout {
    /// Rendering width.
    width: usize,
    /// Screen-relative row.
    row: usize,
    /// Screen-relative column.
    col: usize,
    /// Optional truncation height.
    max_height: Option<usize>,
}

/// One rendered overlay with its first width and final placement.
struct Rendered {
    /// Rendered and height-truncated lines.
    lines: Vec<String>,
    /// First-pass rendering width and final position.
    layout: Layout,
}

/// Parses only ASCII decimal percentage syntax, without trimming.
fn percentage(text: &str) -> Option<f64> {
    let digits = text.strip_suffix('%')?;
    let valid = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    let accepted = digits.split_once('.').map_or_else(
        || valid(digits),
        |(whole, fraction)| valid(whole) && valid(fraction),
    );
    accepted.then(|| digits.parse::<f64>().unwrap_or(f64::INFINITY))
}

/// Floors a fractional span and saturates at the signed cell boundary.
fn floor_cells(value: f64) -> isize {
    format!("{:.0}", value.floor())
        .parse()
        .unwrap_or(isize::MAX)
}

/// Converts a signed span without a lossy integer cast.
fn float_span(span: isize) -> f64 {
    span.to_string().parse().unwrap_or(f64::INFINITY)
}

/// Resolves cells or percentage sizes against `span`.
fn size(value: Option<&SizeValue>, span: isize) -> Option<isize> {
    match value? {
        SizeValue::Cells(cells) => Some(*cells),
        SizeValue::Percentage(text) => {
            percentage(text).map(|percent| floor_cells(float_span(span) * percent / 100.0))
        }
    }
}

/// Saturating conversion from native dimensions to signed layout arithmetic.
fn signed(value: usize) -> isize {
    isize::try_from(value).unwrap_or(isize::MAX)
}

/// Converts a nonnegative layout result to a native dimension.
fn cells(value: isize) -> usize {
    usize::try_from(value).unwrap_or(0)
}

/// Resolves coordinates without narrowing before offsets and clipping.
fn position(value: Option<&SizeValue>, free: isize, margin: isize, anchor: isize) -> f64 {
    match value {
        Some(SizeValue::Cells(value)) => float_span(*value),
        Some(SizeValue::Percentage(text)) => {
            float_span(margin)
                + percentage(text).map_or(float_span(free.div_euclid(2)), |percent| {
                    (float_span(free.max(0)) * (percent / 100.0)).floor()
                })
        }
        None => float_span(margin) + float_span(anchor),
    }
}

/// Returns nonnegative top, right, bottom and left margins.
fn margins(options: &OverlayOptions) -> [isize; 4] {
    let sides = match options.margin {
        Some(OverlayMarginValue::Uniform(value)) => return [value.max(0); 4],
        Some(OverlayMarginValue::Sides(sides)) => sides,
        None => OverlayMargin::default(),
    };
    [sides.top, sides.right, sides.bottom, sides.left].map(|value| value.unwrap_or(0).max(0))
}

/// Resolves the nine anchors as row and column offsets within available space.
fn anchor_offsets(anchor: OverlayAnchor, free_height: isize, free_width: isize) -> (isize, isize) {
    use OverlayAnchor::{
        BottomCenter, BottomLeft, BottomRight, Center, LeftCenter, RightCenter, TopCenter, TopLeft,
        TopRight,
    };
    let row = match anchor {
        TopLeft | TopCenter | TopRight => 0,
        BottomLeft | BottomCenter | BottomRight => free_height,
        LeftCenter | Center | RightCenter => free_height.div_euclid(2),
    };
    let col = match anchor {
        TopLeft | LeftCenter | BottomLeft => 0,
        TopRight | RightCenter | BottomRight => free_width,
        TopCenter | Center | BottomCenter => free_width.div_euclid(2),
    };
    (row, col)
}

/// Resolves sizes against terminal dimensions and placement against the remaining spans.
fn resolve_overlay_layout(
    options: &OverlayOptions,
    height: usize,
    dimensions: (usize, usize),
) -> Layout {
    let (term_width, term_height) = (signed(dimensions.0), signed(dimensions.1));
    let [top, right, bottom, left] = margins(options);
    let available_width = term_width.saturating_sub(left).saturating_sub(right).max(1);
    let available_height = term_height
        .saturating_sub(top)
        .saturating_sub(bottom)
        .max(1);
    let width = size(options.width.as_ref(), term_width)
        .unwrap_or(80.min(available_width))
        .max(options.min_width.unwrap_or(isize::MIN))
        .clamp(1, available_width);
    let max_height = size(options.max_height.as_ref(), term_height)
        .map(|height| height.clamp(1, available_height));
    let effective_height = signed(height).min(max_height.unwrap_or(isize::MAX));
    let free_height = available_height.saturating_sub(effective_height);
    let free_width = available_width.saturating_sub(width);
    let (anchor_row, anchor_col) = anchor_offsets(
        options.anchor.unwrap_or(OverlayAnchor::Center),
        free_height,
        free_width,
    );
    let row = (position(options.row.as_ref(), free_height, top, anchor_row)
        + float_span(options.offset_y.unwrap_or(0)))
    .min(float_span(
        term_height
            .saturating_sub(bottom)
            .saturating_sub(effective_height),
    ))
    .max(float_span(top));
    let col = (position(options.col.as_ref(), free_width, left, anchor_col)
        + float_span(options.offset_x.unwrap_or(0)))
    .min(float_span(
        term_width.saturating_sub(right).saturating_sub(width),
    ))
    .max(float_span(left));
    Layout {
        width: cells(width),
        row: cells(floor_cells(row)),
        col: cells(floor_cells(col)),
        max_height: max_height.map(cells),
    }
}

/// Places one text line, leaving image base lines untouched.
fn composite_line_at(
    base: &str,
    overlay: &str,
    start: usize,
    width: usize,
    total: usize,
) -> String {
    if is_image_line(base) {
        return base.to_owned();
    }
    let base = expand_tabs(base);
    let overlay = expand_tabs(overlay);
    let after_start = start.saturating_add(width);
    let base = extract_segments(
        &base,
        start,
        after_start,
        total.saturating_sub(after_start),
        true,
    );
    let overlay = slice_with_width(&overlay, 0, width, true);
    let before_pad = start.saturating_sub(base.before_width);
    let overlay_pad = width.saturating_sub(overlay.width);
    let after_target = total
        .saturating_sub(start.max(base.before_width))
        .saturating_sub(width.max(overlay.width));
    let result = format!(
        "{}{}{SEGMENT_RESET}{}{}{SEGMENT_RESET}{}{}",
        base.before,
        " ".repeat(before_pad),
        overlay.text,
        " ".repeat(overlay_pad),
        base.after,
        " ".repeat(after_target.saturating_sub(base.after_width))
    );
    if visible_width(&result) > total {
        slice_by_column(&result, 0, total, true)
    } else {
        result
    }
}

impl TUI {
    /// Renders a retained visibility selection before composing it onto a viewport-sized buffer.
    pub(super) fn composite_overlays(
        &self,
        mut lines: Vec<String>,
        dimensions: (usize, usize),
    ) -> Vec<String> {
        let length = self.shared.overlays.borrow().len();
        if length == 0 {
            return lines;
        }
        let mut entries: Vec<_> = (0..length)
            .filter_map(|index| self.overlay_at(index))
            .filter(|entry| self.overlay_visible(entry))
            .collect();
        entries.sort_by_key(|entry| entry.order.get());
        let rendered: Vec<_> = entries
            .into_iter()
            .map(|entry| {
                let mut layout = resolve_overlay_layout(&entry.options.borrow(), 0, dimensions);
                let mut lines = entry.capture.component.render(layout.width);
                if let Some(height) = layout.max_height {
                    lines.truncate(height);
                }
                let position =
                    resolve_overlay_layout(&entry.options.borrow(), lines.len(), dimensions);
                layout.row = position.row;
                layout.col = position.col;
                Rendered { lines, layout }
            })
            .collect();
        let working_height = rendered
            .iter()
            .map(|entry| entry.layout.row.saturating_add(entry.lines.len()))
            .fold(lines.len().max(dimensions.1), usize::max);
        lines.resize(working_height, String::new());
        let viewport_start = working_height.saturating_sub(dimensions.1);
        for entry in rendered {
            entry.composite(&mut lines, viewport_start, dimensions.0);
        }
        lines
    }
}

impl Rendered {
    /// Composes this rendered overlay into the current viewport buffer.
    fn composite(&self, lines: &mut [String], viewport_start: usize, width: usize) {
        for (index, overlay) in self.lines.iter().enumerate() {
            let row = viewport_start
                .saturating_add(self.layout.row)
                .saturating_add(index);
            if let Some(base) = lines.get_mut(row) {
                *base = composite_line_at(base, overlay, self.layout.col, self.layout.width, width);
            }
        }
    }
}
