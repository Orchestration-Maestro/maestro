//! Spreadsheet serialization through the public application boundary.
#![cfg(test)]
use crate::support;
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
            let sheet = book.sheets[&expected.name].as_ref().unwrap();
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
        let actual = &book.sheets[sheet_name].as_ref().unwrap().cells[&address];
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

#[test]
fn workbook_written_formula_escapes_decode_once() {
    use maestro_app::browser::utils::attachment_utils::{AttachmentSource, load_attachment};
    let bytes = include_bytes!("fixtures/spreadsheet/formula-escapes.xlsx");
    let mut format = Ssf::default();
    let mut book = ssf::read(bytes, "escapes.xlsx", &mut format).unwrap();
    let sheet = book.sheets.get_mut("Escapes").unwrap().as_mut().unwrap();
    for (address, expected) in [("A1", "\"a\rb\""), ("B1", "\"a\rb\""), ("C1", "\"a\nb\"")] {
        assert_eq!(sheet.cells[address].f.as_deref(), Some(expected));
    }
    let csv = "7,\"=\"\"a\rb\"\"\",\"=\"\"a\nb\"\"\"";
    assert_eq!(ssf::sheet_to_csv(sheet, &format), csv);
    let fetch: maestro_models::Fetch = std::sync::Arc::new(|_| panic!("file needs no fetch"));
    let attachment = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(load_attachment(
            AttachmentSource::File {
                bytes,
                name: "escapes.xlsx",
                mime_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            },
            None,
            &fetch,
        ))
        .unwrap();
    assert_eq!(attachment.extracted_text.as_deref(), Some(format!("<excel filename=\"escapes.xlsx\">\n<sheet name=\"Escapes\" index=\"1\">\n{csv}\n</sheet>\n</excel>").as_str()));
}

#[test]
fn workbook_shared_formulas_preserve_sheet_identifiers() {
    let book = ssf::read(
        include_bytes!("fixtures/spreadsheet/shared-identifiers.xlsx"),
        "shared.xlsx",
        &mut Ssf::default(),
    )
    .unwrap();
    let sheet = book.sheets["Escapes"].as_ref().unwrap();
    for (address, formula) in [
        ("A1", "'Q1'!A1+Q1!$B1+\"A1\"+SUM(C1)"),
        ("A2", "'Q1'!A2+Q1!$B2+\"A1\"+SUM(C2)"),
        ("A3", "'Q1'!A3+Q1!$B3+\"A1\"+SUM(C3)"),
    ] {
        assert_eq!(sheet.cells[address].f.as_deref(), Some(formula));
    }
}

#[test]
fn workbook_shared_formulas_preserve_structured_columns() {
    let book = ssf::read(
        include_bytes!("fixtures/spreadsheet/shared-identifiers.xlsx"),
        "shared.xlsx",
        &mut Ssf::default(),
    )
    .unwrap();
    let sheet = book.sheets["Escapes"].as_ref().unwrap();
    for (address, formula) in [
        ("B1", "Table1[A1]+Table1[[#This Row],[B2]]+A1"),
        ("B2", "Table1[A1]+Table1[[#This Row],[B2]]+A2"),
        ("B3", "Table1[A1]+Table1[[#This Row],[B2]]+A3"),
    ] {
        assert_eq!(sheet.cells[address].f.as_deref(), Some(formula));
    }
}

/// Check retained formulas and their uncached CSV replay through the reader boundary.
fn unicode_shared_formula(sheet_name: &str, anchor: &str, follower: &str) {
    let mut format = Ssf::default();
    let mut book = ssf::read(
        include_bytes!("fixtures/spreadsheet/shared-unicode.xlsx"),
        "unicode.xlsx",
        &mut format,
    )
    .unwrap();
    let sheet = book.sheets.get_mut(sheet_name).unwrap().as_mut().unwrap();
    assert_eq!(sheet.cells["B1"].f.as_deref(), Some(anchor));
    assert_eq!(sheet.cells["B2"].f.as_deref(), Some(follower));
    assert_eq!(
        ssf::sheet_to_csv(sheet, &format),
        format!("7,={anchor}\n,={follower}")
    );
}

#[test]
fn workbook_shared_unicode_sheet_name_keeps_identity() {
    unicode_shared_formula("Q1年", "Q1年!A1", "Q1年!A2");
}

#[test]
fn workbook_shared_unicode_defined_name_prefix_keeps_identity() {
    unicode_shared_formula("Greek", "ΩA1+A1", "ΩA1+A2");
}

#[test]
fn workbook_shared_unicode_defined_name_suffix_keeps_identity() {
    unicode_shared_formula("Suffix", "A1é+A1", "A1é+A2");
}

#[test]
fn legacy_spreadsheet_metadata_uses_existing_extraction_writer() {
    use base64::Engine as _;
    use maestro_app::browser::utils::attachment_utils::{AttachmentSource, load_attachment};
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Case {
        file: String,
        expected: String,
    }
    let cases: Vec<Case> = serde_json::from_str(include_str!(
        "fixtures/ssf/legacy_metadata_attachments.json"
    ))
    .unwrap();
    let fetch: maestro_models::Fetch = std::sync::Arc::new(|_| panic!("file needs no fetch"));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    for case in cases {
        let bytes = crate::workbook_fixture::bytes(&format!("metadata-{}", case.file));
        let attachment = runtime
            .block_on(load_attachment(
                AttachmentSource::File {
                    bytes: &bytes,
                    name: &case.file,
                    mime_type: "application/vnd.ms-excel",
                },
                None,
                &fetch,
            ))
            .unwrap();
        assert_eq!(attachment.file_name, case.file);
        assert_eq!(attachment.size, bytes.len());
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(&attachment.content)
                .unwrap(),
            bytes
        );
        assert_eq!(
            attachment.extracted_text.as_deref(),
            Some(case.expected.as_str())
        );
        // The complete wrapper preserves both cache text and sheet ordering.
        let mut ssf = Ssf::default();
        let mut book = ssf::read(&bytes, &case.file, &mut ssf).unwrap();
        for sheet in book.sheets.values_mut() {
            let sheet = sheet.as_mut().unwrap();
            let before = sheet.cells.clone();
            let csv = ssf::sheet_to_csv(sheet, &ssf);
            assert!(case.expected.contains(&csv));
            assert_eq!(sheet.cells, before);
        }
    }
}

/// Load an in-memory spreadsheet without network access or another executor.
fn extract_workbook(
    bytes: &[u8],
    name: &str,
) -> Result<
    maestro_app::browser::utils::attachment_utils::Attachment,
    maestro_app::browser::utils::attachment_utils::AttachmentError,
> {
    use maestro_app::browser::utils::attachment_utils::{AttachmentSource, load_attachment};
    let fetch: maestro_models::Fetch =
        std::sync::Arc::new(|_| Box::pin(async { panic!("File source must not fetch") }));
    let mut future = Box::pin(load_attachment(
        AttachmentSource::File {
            bytes,
            name,
            mime_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        },
        None,
        &fetch,
    ));
    let std::task::Poll::Ready(result) = std::future::Future::poll(
        future.as_mut(),
        &mut std::task::Context::from_waker(std::task::Waker::noop()),
    ) else {
        panic!("in-memory file must settle synchronously")
    };
    result
}
#[test]
fn spreadsheet_extraction_keeps_empty_failed_sheet_wrapper() {
    use base64::Engine as _;
    for (file, expected) in [
        (
            "mixed-bad-first.xlsx",
            "<excel filename=\"mixed-bad-first.xlsx\">\n<sheet name=\"Data\" index=\"1\">\n\n</sheet>\n<sheet name=\"Good\" index=\"2\">\n42\n</sheet>\n</excel>",
        ),
        (
            "mixed-bad-last.xlsx",
            "<excel filename=\"mixed-bad-last.xlsx\">\n<sheet name=\"Good\" index=\"1\">\n42\n</sheet>\n<sheet name=\"Data\" index=\"2\">\n\n</sheet>\n</excel>",
        ),
        (
            "mixed-partial-bad.xlsx",
            "<excel filename=\"mixed-partial-bad.xlsx\">\n<sheet name=\"Data\" index=\"1\">\n\n</sheet>\n<sheet name=\"Good\" index=\"2\">\n42\n</sheet>\n</excel>",
        ),
        (
            "mixed-literal-error.xlsx",
            "<excel filename=\"mixed-literal-error.xlsx\">\n<sheet name=\"Data\" index=\"1\">\n\n</sheet>\n<sheet name=\"Good\" index=\"2\">\n42\n</sheet>\n</excel>",
        ),
        (
            "mixed-empty-valid.xlsx",
            "<excel filename=\"mixed-empty-valid.xlsx\">\n<sheet name=\"Data\" index=\"1\">\n\n</sheet>\n<sheet name=\"Good\" index=\"2\">\n\n</sheet>\n</excel>",
        ),
    ] {
        let bytes = crate::workbook_fixture::bytes(file);
        let result = extract_workbook(&bytes, file).unwrap();
        assert_eq!(result.extracted_text.as_deref(), Some(expected));
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(&result.content)
                .unwrap(),
            bytes
        );
    }
    let bytes = crate::workbook_fixture::bytes("all-corrupt-two.xlsx");
    let error = extract_workbook(&bytes, "all-corrupt-two.xlsx").unwrap_err();
    assert_eq!(
        error.to_string(),
        "Failed to process Excel: all-corrupt-two.xlsx: xl/worksheets/sheet1.xml: shared string 3"
    );
}

/// Shared strings amplify four individually bounded sheets beyond the output bound.
#[test]
#[cfg(not(target_arch = "wasm32"))]
fn spreadsheet_extracted_output_past_the_bound_fails() {
    use maestro_app::browser::ssf::WorkbookError;
    use maestro_app::browser::utils::attachment_utils::AttachmentError;
    use std::io::{Cursor, Write};

    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (path, text) in [
        (
            "[Content_Types].xml",
            "<Types><Override ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml\" PartName=\"/xl/sharedStrings.xml\"/></Types>",
        ),
        (
            "xl/workbook.xml",
            "<workbook><sheets><sheet name=\"One\"/><sheet name=\"Two\"/><sheet name=\"Three\"/><sheet name=\"Four\"/></sheets></workbook>",
        ),
    ] {
        writer.start_file(path, options).unwrap();
        writer.write_all(text.as_bytes()).unwrap();
    }
    writer.start_file("xl/sharedStrings.xml", options).unwrap();
    writer.write_all(b"<sst><si><t>").unwrap();
    writer.write_all(&vec![b'x'; 1 << 20]).unwrap();
    writer.write_all(b"</t></si></sst>").unwrap();
    for index in 1..=4 {
        writer
            .start_file(format!("xl/worksheets/sheet{index}.xml"), options)
            .unwrap();
        writer.write_all(b"<worksheet><sheetData>").unwrap();
        for row in 1..=129 {
            write!(
                writer,
                "<row r=\"{row}\"><c r=\"A{row}\" t=\"s\"><v>0</v></c></row>"
            )
            .unwrap();
        }
        writer.write_all(b"</sheetData></worksheet>").unwrap();
    }
    let bytes = writer.finish().unwrap().into_inner();
    let error = extract_workbook(&bytes, "large.xlsx").unwrap_err();
    assert_eq!(
        error.to_string(),
        "Failed to process Excel: extracted text exceeds 512 MiB"
    );
    assert!(
        !matches!(error, AttachmentError::Excel(WorkbookError::Reader(_))),
        "output overflow was classified as a value-reader failure"
    );
}
