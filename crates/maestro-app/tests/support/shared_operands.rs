//! Authored formula operands observed through workbook, CSV and attachment APIs.
use maestro_app::browser::ssf::{self, Ssf};
use serde::Deserialize;
use std::collections::BTreeSet;

/// Pinned-writer workbook with shared records encoded without formula changes.
const BYTES: &[u8] = include_bytes!("../fixtures/spreadsheet/operand-grammar.xlsx");

/// Independent worked expectations and annotated coordinate leaves.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Named grammar witnesses.
    named: Vec<Case>,
    /// Opaque spans and explicitly marked coordinate leaves.
    fragments: Vec<Vec<Piece>>,
    /// Production boundaries and displacement properties.
    boundary: Vec<Case>,
}

/// One public reader input and its independently specified follower.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Diagnostic name.
    id: String,
    /// Template location.
    start: String,
    /// Follower location.
    end: String,
    /// Authored template.
    source: String,
    /// Required displaced text.
    expected: String,
}

/// A coordinate leaf or an opaque fragment, annotated before context rendering.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Piece {
    /// Source bytes that displacement cannot change.
    Raw(String),
    /// Source and worked shifted A1 coordinate.
    Cell([String; 2]),
}

/// Load strict readable recordings.
fn corpus() -> Corpus {
    serde_json::from_str(include_str!("../fixtures/spreadsheet/operand-grammar.json")).unwrap()
}

/// Render an annotated fragment without parsing any formula text.
fn fragment(pieces: &[Piece], shifted: bool) -> String {
    pieces
        .iter()
        .map(|p| match p {
            Piece::Raw(text) => text.as_str(),
            Piece::Cell(pair) => pair[usize::from(shifted)].as_str(),
        })
        .collect()
}

/// Apply the four independently specified expression contexts to each fragment.
fn generated(data: &Corpus) -> Vec<Case> {
    let mut cases = Vec::new();
    for (i, pieces) in data.fragments.iter().enumerate() {
        for (j, (prefix, suffix, expected_suffix)) in [
            ("", "+A1", "+B2"),
            ("SUM(", ",A1)", ",B2)"),
            ("IF(TRUE,", ",A1)", ",B2)"),
            ("", "&\"A1\"", "&\"A1\""),
        ]
        .into_iter()
        .enumerate()
        {
            cases.push(Case {
                id: format!("generated-{i}-{j}"),
                start: "B2".into(),
                end: "C3".into(),
                source: format!("{prefix}{}{suffix}", fragment(pieces, false)),
                expected: format!("{prefix}{}{expected_suffix}", fragment(pieces, true)),
            });
        }
    }
    cases
}

/// The exact ordered sheet inputs, including byte-identical zero displacement.
fn cases() -> Vec<Case> {
    let data = corpus();
    let mut all = data.named.clone();
    all.extend(generated(&data));
    let zero: Vec<_> = all
        .iter()
        .enumerate()
        .map(|(i, case)| Case {
            id: format!("zero-{i}"),
            start: "B2".into(),
            end: "B2".into(),
            source: case.source.clone(),
            expected: case.source.replace("\r\n", "\n"),
        })
        .collect();
    all.extend(data.boundary);
    all.extend(zero);
    all
}

/// Expected default CSV over the writer's declared four-by-four rectangle.
fn expected_csv(case: &Case) -> String {
    let mut rows = vec![vec![String::new(); 4]; 4];
    for (address, text) in [(&case.start, &case.source), (&case.end, &case.expected)] {
        let column = usize::from(address.as_bytes()[0] - b'A');
        let row = usize::from(address.as_bytes()[1] - b'1');
        let text = format!("={}", text.replace("\r\n", "\n"));
        rows[row][column] = if text.contains([',', '"', '\n']) {
            format!("\"{}\"", text.replace('"', "\"\""))
        } else {
            text
        };
    }
    rows.iter()
        .map(|row| row.join(","))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Accumulate every failing row so the red run records each branch independently.
fn failures(select: impl Fn(usize, &Case) -> bool) -> Vec<String> {
    let mut format = Ssf::default();
    let mut book = ssf::read(BYTES, "operands.xlsx", &mut format).unwrap();
    let mut failures = Vec::new();
    let mut selected = 0;
    for (i, case) in cases().iter().enumerate().filter(|(i, c)| select(*i, c)) {
        selected += 1;
        let sheet = book
            .sheets
            .get_mut(&format!("Case{i}"))
            .unwrap()
            .as_mut()
            .unwrap();
        let actual = sheet.cells[&case.end].f.as_deref();
        if actual != Some(case.expected.as_str()) {
            failures.push(format!(
                "{}: expected {:?}, got {:?}",
                case.id, case.expected, actual
            ));
        }
        if case.start != case.end {
            assert_eq!(
                sheet.cells[&case.start].f.as_deref(),
                Some(case.source.replace("\r\n", "\n").as_str()),
                "{} anchor",
                case.id
            );
        }
        assert!(
            matches!(sheet.cells[&case.end].v, ssf::CellValue::Missing),
            "{} uncached",
            case.id
        );
        if ssf::sheet_to_csv(sheet, &format) != expected_csv(case) {
            failures.push(format!("{}: uncached CSV differs", case.id));
        }
    }
    assert!(selected > 0, "the requested witness must exist");
    failures
}

/// Give every named input its own independent red/green result.
macro_rules! named {
    ($($test:ident => $id:literal),* $(,)?) => { $(
        #[test]
        fn $test() {
            let errors = failures(|_, c| c.id == $id);
            assert!(errors.is_empty(), "{}", errors.join("\n"));
        }
    )* };
}

named! {
    workbook_shared_quoted_sheet => "quoted-sheet",
    workbook_shared_quoted_escaped_unicode_sheet => "quoted-escaped-unicode-sheet",
    workbook_shared_structured_column => "structured-column",
    workbook_shared_structured_nested => "structured-nested",
    workbook_shared_cjk_sheet => "cjk-sheet",
    workbook_shared_greek_name => "greek-name",
    workbook_shared_accent_name => "accent-name",
    workbook_shared_dynamic_range => "dynamic-range",
    workbook_shared_named_range => "named-range",
    workbook_shared_spaced_range => "spaced-range",
    workbook_shared_three_dimensional_sheet => "three-dimensional-sheet",
    workbook_shared_string => "string",
    workbook_shared_escaped_string => "escaped-string",
    workbook_shared_whole_column => "whole-column",
    workbook_shared_whole_row => "whole-row",
    workbook_shared_absolute_whole_column => "absolute-whole-column",
    workbook_shared_absolute_whole_row => "absolute-whole-row",
    workbook_shared_lowercase => "lowercase",
    workbook_shared_mixed_absolute => "mixed-absolute",
    workbook_shared_function_cell_shape => "function-cell-shape",
    workbook_shared_outside_grid_name => "outside-grid-name",
    workbook_shared_qualified_name => "qualified-name",
    workbook_shared_external_qualifier => "external-qualifier",
    workbook_shared_reference_union => "reference-union",
    workbook_shared_reference_intersection => "reference-intersection",
    workbook_shared_rc_in_xlsx => "rc-in-xlsx",
    workbook_shared_array_text => "array-text",
    workbook_shared_numeric_exponent => "numeric-exponent",
    workbook_shared_operator_range_call => "operator-range-call",
    workbook_shared_parenthesized_range => "parenthesized-range",
    workbook_shared_qualified_range => "qualified-range",
    workbook_shared_intersection_cell => "intersection-cell",
    workbook_shared_error_na_division => "error-na-division",
    workbook_shared_error_divzero_division => "error-divzero-division",
    workbook_shared_error_name_division => "error-name-division",
    workbook_shared_error_getting_data_division => "error-getting-data-division",
    workbook_shared_spill_division => "spill-division",
    workbook_shared_intersection_range => "intersection-range",
    workbook_shared_intersection_axis => "intersection-axis",
    workbook_shared_quarterly_qualifier => "quarterly-qualifier",
    workbook_shared_ambiguous_colon_qualifier => "ambiguous-colon-qualifier",
    workbook_shared_mixed_case_coordinate => "mixed-case-coordinate",
    workbook_shared_sheet_punctuation => "unquoted-sheet-punctuation",
    workbook_shared_decomposed_name => "decomposed-name",
    workbook_shared_overflow_row => "overflow-row",
    workbook_shared_overflow_column => "overflow-column",
    workbook_shared_overflow_range => "overflow-range",
}

#[test]
fn workbook_shared_generated_operand_contexts() {
    let data = corpus();
    let generated = generated(&data);
    assert_eq!(data.fragments.len(), 40);
    assert_eq!(generated.len(), 160);
    assert_eq!(
        generated
            .iter()
            .map(|c| &c.source)
            .collect::<BTreeSet<_>>()
            .len(),
        160
    );
    let errors = failures(|i, _| (30..190).contains(&i));
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn workbook_shared_boundaries_and_displacement_properties() {
    let errors = failures(|i, _| i >= 190);
    assert!(errors.is_empty(), "{}", errors.join("\n"));
    let all = cases();
    let expected = |id| all.iter().find(|c| c.id == id).unwrap().expected.as_str();
    let source = |id| all.iter().find(|c| c.id == id).unwrap().source.as_str();
    assert_eq!(expected("composition-first"), source("composition-second"));
    assert_eq!(
        expected("composition-second"),
        expected("composition-direct")
    );
    assert_eq!(expected("inverse"), source("composition-first"));
}

#[test]
fn workbook_shared_opaque_spans_and_absolute_bindings() {
    let data = corpus();
    let book = ssf::read(BYTES, "operands.xlsx", &mut Ssf::default()).unwrap();
    for (i, pieces) in data.fragments.iter().enumerate() {
        for j in 0..4 {
            let text = book.sheets[&format!("Case{}", 30 + i * 4 + j)]
                .as_ref()
                .unwrap()
                .cells["C3"]
                .f
                .as_deref()
                .unwrap();
            assert_eq!(
                text.matches('$').count(),
                fragment(pieces, false).matches('$').count()
            );
            for piece in pieces {
                assert_piece(text, piece);
            }
        }
    }
}

/// Check opaque spans and independently annotated coordinate leaves.
fn assert_piece(text: &str, piece: &Piece) {
    match piece {
        Piece::Raw(raw) => {
            assert!(text.contains(raw), "opaque span {raw:?} in {text:?}");
        }
        Piece::Cell(pair) => {
            assert!(
                text.contains(&pair[1]),
                "coordinate leaf {:?} in {text:?}",
                pair[1]
            );
        }
    }
}

#[test]
fn workbook_shared_operand_attachment_composition() {
    use maestro_app::browser::utils::attachment_utils::{AttachmentSource, load_attachment};
    use std::fmt::Write;
    let mut expected = String::new();
    for (i, c) in cases().iter().enumerate() {
        write!(
            expected,
            "\n<sheet name=\"Case{i}\" index=\"{}\">\n{}\n</sheet>",
            i + 1,
            expected_csv(c)
        )
        .unwrap();
    }
    let fetch: maestro_models::Fetch = std::sync::Arc::new(|_| panic!("file needs no fetch"));
    let attachment = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(load_attachment(
            AttachmentSource::File {
                bytes: BYTES,
                name: "operands.xlsx",
                mime_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            },
            None,
            &fetch,
        ))
        .unwrap();
    assert_eq!(
        attachment.extracted_text.as_deref(),
        Some(format!("<excel filename=\"operands.xlsx\">{expected}\n</excel>").as_str())
    );
}
