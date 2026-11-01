//! Whole terminal cells from a numeric width.

/// The whole cells a numeric width occupies: its floor, at least one cell.
pub(crate) fn whole_cell_width(width: f64) -> u32 {
    format!("{:.0}", width.floor().max(1.0))
        .parse()
        .unwrap_or(u32::MAX)
}
