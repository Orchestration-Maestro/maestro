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

#[test]
fn sheet_tables_share_raw_date_spelling_with_failed_cell_formats() {
    use maestro_app::browser::ssf::*;
    let ssf = Ssf::default();
    let _ = ssf.load("[", Some(999));
    let mut cell = CellObject {
        t: Some(ExcelDataType::Date),
        v: CellValue::Date("2026-10-16T12:34:56.789Z".parse().unwrap()),
        z: Some(NumberFormat::Code("[".into())),
        xf: Some(Xf { num_fmt_id: 999 }),
        ..Default::default()
    };
    let CellValue::Text(raw) =
        format_cell(Some(&mut cell), None, &CellFormatOptions::default(), &ssf)
    else {
        panic!("raw date must be text")
    };
    assert_eq!(raw, "2026-10-16T12:34:56.789Z");
    let mut sheet = WorkSheet {
        cells: [("A1".into(), cell)].into_iter().collect(),
        range: Some(decode_range("A1").unwrap()),
        merges: Vec::new(),
    };
    let html = sheet_to_html(&mut sheet, "date", &ssf);
    assert!(
        html.contains(&format!("data-t=\"d\" data-v=\"{raw}\"")),
        "{html}"
    );
}

#[test]
fn sheet_tables_keep_numeric_format_selector_metadata() {
    use maestro_app::browser::ssf::*;
    let mut sheet = WorkSheet {
        cells: [(
            "A1".into(),
            CellObject {
                t: Some(ExcelDataType::Number),
                v: CellValue::Number(1.25),
                z: Some(NumberFormat::Id(2)),
                ..Default::default()
            },
        )]
        .into_iter()
        .collect(),
        range: Some(decode_range("A1").unwrap()),
        merges: Vec::new(),
    };
    assert_eq!(
        sheet_to_html(&mut sheet, "", &Ssf::default()),
        "<table><tr><td data-t=\"n\" data-v=\"1.25\" data-z=\"2\" id=\"sjs-A1\">1.25</td></tr></table>"
    );
}
