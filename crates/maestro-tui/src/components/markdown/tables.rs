//! Width-aware table layout with a source-row fallback.
use super::{
    Renderer,
    inline::Style,
    parse::{Cells, Table},
};
use crate::text::utils::{
    extract_ansi_code, is_whitespace_scalar, visible_width, wrap_text_with_ansi,
};

/// Widest word that sets a column minimum; longer words wrap inside the cell.
const MAX_UNBROKEN_WORD_WIDTH: usize = 30;

impl Renderer<'_> {
    /// Draws bordered rows, or the wrapped source rows when each column cannot get one cell.
    pub(super) fn table(&self, table: &Table, width: usize, style: Style<'_>) -> Vec<String> {
        let columns = table.header.len();
        let Some(available) = width
            .checked_sub(3 * columns + 1)
            .filter(|available| *available >= columns)
        else {
            return wrap_text_with_ansi(&table.authored, width);
        };
        let widths = self.widths(table, style, available);
        let separator = border(['├', '┼', '┤'], &widths);
        let mut lines = vec![border(['┌', '┬', '┐'], &widths)];
        lines.extend(cell_lines(
            &self.row(&table.header, style),
            &widths,
            |text| (self.theme.bold)(text),
        ));
        lines.push(separator.clone());
        for (index, row) in table.rows.iter().enumerate() {
            if index > 0 {
                lines.push(separator.clone());
            }
            lines.extend(cell_lines(&self.row(row, style), &widths, str::to_owned));
        }
        lines.push(border(['└', '┴', '┘'], &widths));
        lines
    }
}

impl Renderer<'_> {
    /// Renders each cell of one row in source order.
    fn row(&self, cells: &Cells, style: Style<'_>) -> Vec<String> {
        cells.iter().map(|cell| self.inline(cell, style)).collect()
    }

    /// Measures every cell once, header first, and fits the columns to `available`.
    fn widths(&self, table: &Table, style: Style<'_>, available: usize) -> Vec<usize> {
        let header = self.row(&table.header, style);
        let rows: Vec<Vec<String>> = table.rows.iter().map(|row| self.row(row, style)).collect();
        column_widths(&header, &rows, available)
    }
}

/// Chooses natural widths when they fit, otherwise shares `available` cells.
fn column_widths(header: &[String], rows: &[Vec<String>], available: usize) -> Vec<usize> {
    let mut natural: Vec<usize> = header
        .iter()
        .map(|text| visible_width(text).max(1))
        .collect();
    let mut words: Vec<usize> = header
        .iter()
        .map(|text| longest_word(text).max(1))
        .collect();
    for row in rows {
        for ((text, natural), words) in row.iter().zip(&mut natural).zip(&mut words) {
            *natural = (*natural).max(visible_width(text));
            *words = (*words).max(longest_word(text));
        }
    }
    let minimum = if words.iter().sum::<usize>() > available {
        shrink(&words, available)
    } else {
        words
    };
    if natural.iter().sum::<usize>() <= available {
        return natural
            .iter()
            .zip(&minimum)
            .map(|(natural, minimum)| *natural.max(minimum))
            .collect();
    }
    let potential = natural
        .iter()
        .zip(&minimum)
        .map(|(natural, minimum)| natural.saturating_sub(*minimum))
        .sum();
    let extra = available.saturating_sub(minimum.iter().sum());
    let mut widths: Vec<usize> = natural
        .iter()
        .zip(&minimum)
        .map(|(natural, minimum)| {
            minimum + proportional(natural.saturating_sub(*minimum), potential, extra)
        })
        .collect();
    let remaining = available.saturating_sub(widths.iter().sum());
    widths
        .iter_mut()
        .zip(&natural)
        .filter(|(width, natural)| **width < **natural)
        .take(remaining)
        .for_each(|(width, _)| *width += 1);
    widths
}

/// Gives each column one cell and shares the rest by longest-word weight, remainder leftmost.
fn shrink(words: &[usize], available: usize) -> Vec<usize> {
    let remaining = available.saturating_sub(words.len());
    let total = words.iter().map(|word| word.saturating_sub(1)).sum();
    let mut widths: Vec<usize> = words
        .iter()
        .map(|word| 1 + proportional(word.saturating_sub(1), total, remaining))
        .collect();
    let leftover = available.saturating_sub(widths.iter().sum());
    widths
        .iter_mut()
        .take(leftover)
        .for_each(|width| *width += 1);
    widths
}

/// Floors `share / total * extra` as double-precision division then multiplication does.
fn proportional(share: usize, total: usize, extra: usize) -> usize {
    let Some(exact) = share.saturating_mul(extra).checked_div(total) else {
        return 0;
    };
    // The double result is within a few units in the last place of the exact quotient, so
    // its floor is the exact floor, or one below it when the quotient is a whole number.
    if double(share) / double(total) * double(extra) < double(exact) {
        exact - 1
    } else {
        exact
    }
}

/// Converts a cell count to a double, saturating beyond `u32`.
fn double(cells: usize) -> f64 {
    u32::try_from(cells).map_or(f64::from(u32::MAX), f64::from)
}

/// Visible width of the widest whitespace-separated word of the text without its escapes,
/// capped at the unbroken limit.
fn longest_word(text: &str) -> usize {
    let mut visible = String::new();
    let mut rest = text;
    while let Some(scalar) = rest.chars().next() {
        let length = extract_ansi_code(rest, 0).map_or_else(
            || {
                visible.push(scalar);
                scalar.len_utf8()
            },
            |code| code.length,
        );
        rest = &rest[length..];
    }
    visible
        .split(is_whitespace_scalar)
        .map(visible_width)
        .max()
        .unwrap_or(0)
        .min(MAX_UNBROKEN_WORD_WIDTH)
}

/// Joins horizontal rules with the given left, junction and right corners.
fn border([left, junction, right]: [char; 3], widths: &[usize]) -> String {
    let cells: Vec<String> = widths.iter().map(|width| "─".repeat(*width)).collect();
    format!("{left}─{}─{right}", cells.join(&format!("─{junction}─")))
}

/// Wraps each cell to its column and pads every physical row of the table row.
fn cell_lines(
    cells: &[String],
    widths: &[usize],
    decorate: impl Fn(&str) -> String,
) -> Vec<String> {
    let wrapped: Vec<Vec<String>> = cells
        .iter()
        .zip(widths)
        .map(|(text, width)| wrap_text_with_ansi(text, *width))
        .collect();
    let height = wrapped.iter().map(Vec::len).max().unwrap_or(0);
    (0..height)
        .map(|line| {
            let parts: Vec<String> = wrapped
                .iter()
                .zip(widths)
                .map(|(lines, width)| {
                    let text = lines.get(line).map_or("", String::as_str);
                    decorate(&format!(
                        "{text}{}",
                        " ".repeat(width.saturating_sub(visible_width(text)))
                    ))
                })
                .collect();
            format!("│ {} │", parts.join(" │ "))
        })
        .collect()
}
