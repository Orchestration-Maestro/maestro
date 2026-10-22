//! Tables, columns, rows, cells, merges and placeholders.
use super::check;

#[test]
fn table_grid_columns_and_spans_render() {
    check("table-grid");
}

#[test]
fn vertical_merges_extend_the_first_cell() {
    check("table-vertical-merge");
}

#[test]
fn grid_offsets_emit_placeholder_cells() {
    check("table-grid-offsets");
}

#[test]
fn table_cell_borders_are_copied_to_cells() {
    check("table-cell-and-table-styles");
}

#[test]
fn nested_tables_keep_independent_merge_state() {
    check("nested-tables");
}

#[test]
fn tables_in_headers_keep_their_own_merge_state() {
    check("table-in-header");
}
