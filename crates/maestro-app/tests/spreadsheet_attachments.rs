//! Spreadsheet serialization through the public application boundary.
#![cfg(test)]
mod support {
    pub mod attachment_observation;
    pub mod spreadsheet;
}
use maestro_app::browser::ssf::{self, Ssf};
use support::spreadsheet::{corpus, equal_cell, worksheet};

fn csv_suite(name: &str) {
    let cases: Vec<_> = corpus()
        .csv
        .into_iter()
        .filter(|c| c.test == name)
        .collect();
    assert!(!cases.is_empty());
    for case in cases {
        let mut sheet = worksheet(case.input);
        assert_eq!(
            ssf::sheet_to_csv(&mut sheet, &Ssf::default()),
            case.expected.csv,
            "{}",
            case.id
        );
        let expected = worksheet(case.expected.cells);
        assert_eq!(sheet.range, expected.range);
        assert_eq!(
            sheet.cells.keys().collect::<Vec<_>>(),
            expected.cells.keys().collect::<Vec<_>>()
        );
        for (address, cell) in expected.cells {
            equal_cell(&sheet.cells[&address], &cell);
        }
    }
}
#[test]
fn csv_range_and_empty_sheets() {
    csv_suite("csv_range_and_empty_sheets");
}
#[test]
fn csv_quotes_separators_without_trimming() {
    csv_suite("csv_quotes_separators_without_trimming");
}
#[test]
fn csv_initial_identifier_protection() {
    csv_suite("csv_initial_identifier_protection");
}
#[test]
fn csv_cached_text_and_scalar_fallback() {
    csv_suite("csv_cached_text_and_scalar_fallback");
}
#[test]
fn csv_blank_rows_and_trailing_columns() {
    csv_suite("csv_blank_rows_and_trailing_columns");
}
#[test]
fn csv_formula_fallback_and_array_suppression() {
    csv_suite("csv_formula_fallback_and_array_suppression");
}

/// Compare owned cells after the native reader and XML borrows have gone away.
#[test]
fn workbook_formula_metadata_keeps_cached_and_missing_values() {
    for recording in corpus().workbooks {
        let mut ssf = Ssf::default();
        let book = ssf::read(
            &support::spreadsheet::bytes(&recording.file),
            "formula.xlsx",
            &mut ssf,
        )
        .unwrap();
        assert_eq!(
            book.sheets.keys().collect::<Vec<_>>(),
            recording
                .expected
                .iter()
                .map(|s| &s.name)
                .collect::<Vec<_>>()
        );
        for expected in recording.expected {
            let sheet = &book.sheets[&expected.name];
            assert_eq!(sheet.range.map(ssf::encode_range), expected.range);
            assert!(sheet.merges.is_empty());
            assert_eq!(
                sheet.cells.keys().collect::<Vec<_>>(),
                expected.cells.keys().collect::<Vec<_>>()
            );
            let name = expected.name;
            for (address, cell) in expected.cells {
                let actual = &sheet.cells[&address];
                let expected = cell.owned();
                check_metadata(actual, &expected, &name, &address);
            }
        }
    }
}

/// Compare scalar state and metadata not owned by the specialized views.
fn check_metadata(actual: &ssf::CellObject, expected: &ssf::CellObject, name: &str, address: &str) {
    assert_eq!(actual.t, expected.t);
    assert_eq!(actual.v, expected.v);
    assert_eq!(actual.w, expected.w);
    if name == "Cached" || (name == "Scalars" && address.ends_with('1')) {
        assert_eq!(actual.f, expected.f);
    }
    if name != "Array" {
        assert_eq!(actual.array_formula, expected.array_formula);
    }
}

/// Compare only the formula metadata owned by this reader view.
fn formula_view(sheet_name: &str, select: impl Fn(&str) -> bool) {
    let recording = corpus().workbooks.pop().unwrap();
    let book = ssf::read(
        &support::spreadsheet::bytes(&recording.file),
        "formulas.xlsx",
        &mut Ssf::default(),
    )
    .unwrap();
    let expected = recording
        .expected
        .into_iter()
        .find(|s| s.name == sheet_name)
        .unwrap();
    for (address, cell) in expected
        .cells
        .into_iter()
        .filter(|(address, _)| select(address))
    {
        let actual = &book.sheets[sheet_name].cells[&address];
        assert_eq!(actual.f, cell.f);
        if sheet_name == "Array" {
            assert_eq!(actual.array_formula, cell.array_formula);
        }
    }
}
#[test]
fn workbook_shared_formulas_are_sheet_local() {
    formula_view("Shared", |_| true);
}
#[test]
fn workbook_array_regions_suppress_missing_formula_replay() {
    formula_view("Array", |_| true);
}
#[test]
fn workbook_formula_text_decodes_entities_and_function_prefixes() {
    formula_view("Scalars", |address| address.ends_with('2'));
}

/// Replay complete source results through the injected transport.
fn attachment_suite(name: &str) {
    use maestro_app::browser::utils::attachment_utils::{AttachmentSource, load_attachment};
    use maestro_models::{Fetch, HttpResponse};
    use std::sync::{Arc, Mutex};
    let rows: Vec<_> = corpus()
        .attachments
        .into_iter()
        .filter(|c| c.test == name)
        .collect();
    assert!(!rows.is_empty());
    for row in rows {
        let bytes = support::spreadsheet::bytes(&row.input.file);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let captured = calls.clone();
        let body = bytes.clone();
        let mime = row.input.mime.clone().unwrap_or_default();
        let response_mime = mime.clone();
        let fetch: Fetch = Arc::new(move |request| {
            assert_eq!(request.method, "GET");
            assert!(
                request.headers.is_empty() && request.body.is_empty() && request.signal.is_none()
            );
            captured.lock().unwrap().push(request.url);
            let response = HttpResponse {
                status: 200,
                status_text: "OK".into(),
                headers: [("content-type".into(), response_mime.clone())].into(),
                body: Box::pin(futures_util::stream::iter([Ok(body.clone())])),
            };
            Box::pin(async move { Ok(response) })
        });
        let source = match row.input.source.as_str() {
            "file" => AttachmentSource::File {
                bytes: &bytes,
                name: row.input.name.as_deref().unwrap(),
                mime_type: &mime,
            },
            "blob" => AttachmentSource::Blob {
                bytes: &bytes,
                mime_type: &mime,
            },
            "buffer" => AttachmentSource::Buffer(&bytes),
            "url" => AttachmentSource::Url(row.input.url.as_deref().unwrap()),
            _ => panic!("unexpected source"),
        };
        let result = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(load_attachment(
                source,
                row.input.r#override.as_deref(),
                &fetch,
            ));
        check_failure(&result, &row.id);
        let actual = support::attachment_observation::observed(result);
        let expected = expected_attachment(row.expected);
        assert_eq!(actual, expected, "{}", row.id);
        assert_eq!(*calls.lock().unwrap(), row.fetch_calls);
    }
}
/// Project native errors to the recorded failure category, not engine wording.
fn expected_attachment(mut expected: serde_json::Value) -> serde_json::Value {
    if let Some(diagnostics) = expected
        .as_object_mut()
        .unwrap()
        .shift_remove("diagnostics")
    {
        assert_eq!(diagnostics, serde_json::json!([]));
    }
    let fields = expected.as_object_mut().unwrap();
    if let Some(error) = fields.shift_remove("error") {
        match error {
            serde_json::Value::String(text) => {
                fields.insert("error".into(), text.strip_prefix("Error: ").unwrap().into());
            }
            serde_json::Value::Object(mut error) => {
                let kind = error.shift_remove("kind").unwrap();
                error.insert("error_kind".into(), kind);
                fields.extend(error);
            }
            _ => panic!("unexpected error recording"),
        }
    }
    expected
}
/// Failures must remain native workbook categories, not unsupported MIME.
fn check_failure(
    result: &Result<
        maestro_app::browser::utils::attachment_utils::Attachment,
        maestro_app::browser::utils::attachment_utils::AttachmentError,
    >,
    id: &str,
) {
    use maestro_app::browser::utils::attachment_utils::AttachmentError;
    use ssf::WorkbookError;
    match id {
        "truncated.xlsx" => assert!(matches!(
            result,
            Err(AttachmentError::Excel(WorkbookError::Archive(_)))
        )),
        "no-styles-part.xlsx" => assert!(matches!(
            result,
            Err(AttachmentError::Excel(WorkbookError::MissingPart(_)))
        )),
        _ => {}
    }
}
#[test]
fn spreadsheet_recognition_and_mime_selection() {
    attachment_suite("spreadsheet_recognition_and_mime_selection");
}

#[test]
fn spreadsheet_attachments_keep_source_fields() {
    attachment_suite("spreadsheet_attachments_keep_source_fields");
}

#[test]
fn spreadsheet_sheet_wrappers_keep_order_and_empty_layout() {
    attachment_suite("spreadsheet_sheet_wrappers_keep_order_and_empty_layout");
}

#[test]
fn spreadsheet_url_sources_use_shared_transport() {
    attachment_suite("spreadsheet_url_sources_use_shared_transport");
}

#[test]
fn spreadsheet_extraction_reuses_formatted_workbook_values() {
    attachment_suite("spreadsheet_extraction_reuses_formatted_workbook_values");
}

#[test]
fn spreadsheet_failures_keep_typed_workbook_sources() {
    attachment_suite("spreadsheet_failures_keep_typed_workbook_sources");
}
