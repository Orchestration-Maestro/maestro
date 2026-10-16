//! Public workbook HTML projection against independently recorded inputs.
#![cfg(test)]
mod spreadsheet_support;
use spreadsheet_support::*;

#[test]
fn sheet_tables_keep_cells_ranges_merges_and_ids() {
    tables(&[
        "no_range",
        "empty_range",
        "offset_sparse",
        "merge_both_axes",
        "merge_overlap_order",
        "merge_outside_view",
        "empty_id",
        "literal_sheet_id",
    ]);
}

#[test]
fn sheet_tables_keep_cache_precedence_and_scalar_spelling() {
    tables(&[
        "typed_scalars",
        "missing_and_null",
        "uncached_error",
        "formatted_and_raw",
        "rich_over_cached",
        "format_attribute",
        "nan",
        "infinity",
        "negative_infinity",
    ]);
}

#[test]
fn sheet_tables_escape_literal_text_and_attributes() {
    tables(&[
        "escape_entities",
        "escape_all_controls",
        "leading_space",
        "trailing_space",
        "bom",
        "nel",
        "nbsp",
        "line_separator",
        "middle_space",
        "literal_sheet_id",
        "quoted_format_attribute",
    ]);
}

#[test]
fn sheet_tables_keep_links_but_disable_executable_targets() {
    tables(&["links", "executable_links", "executable_url_boundaries"]);
}

#[test]
fn workbook_previews_keep_rich_shared_and_inline_cells() {
    workbooks(&[
        "rich.xlsx",
        "rich-styles.xlsx",
        "inline-rich.xlsx",
        "literal-rich.xlsx",
        "two-sheets.xlsx",
        "one-sheet.xlsx",
        "merged.xlsx",
        "quoted-sheet.xlsx",
        "blank-sheet.xlsx",
    ]);
}

#[test]
fn workbook_preview_flags_respect_explicit_off_values() {
    workbooks(&["rich-false-flags.xlsx"]);
}

#[test]
fn workbook_preview_hyperlinks_keep_ranges_fragments_and_stubs() {
    workbooks(&["links.xlsx", "link-ranges.xlsx", "active-links.xlsx"]);
}
