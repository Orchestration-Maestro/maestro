//! Raw legacy record observations before cell projection.
use super::{Encoding, Records, cfb, strings};

fn stream(file: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/workbooks")
        .join(file);
    cfb::workbook_stream(&std::fs::read(path).unwrap()).unwrap()
}

fn formats(file: &str) -> Vec<(u16, String)> {
    let stream = stream(file);
    let mut records = Records::new(&stream);
    let mut formats = Vec::new();
    while let Some(record) = records.next().unwrap() {
        if record.kind == 0x041e {
            let mut payload = record.payload();
            formats.push((
                payload.u16().unwrap(),
                strings::format_string(&mut payload, Encoding::Unicode).unwrap(),
            ));
        }
    }
    formats
}

#[test]
fn format_records_keep_raw_order_duplicates_and_producer_text() {
    assert_eq!(
        formats("legacy-table-overwrite.xls"),
        vec![
            (56, "\"上午/下午 \"hh\"時\"mm\"分\"ss\"秒 \"".to_owned()),
            (164, "\"first\"0".to_owned()),
            (500, "\"high\"0".to_owned()),
            (164, "\"last\"0".to_owned()),
            (168, "\"unused\"0".to_owned()),
        ]
    );
    assert!(formats("legacy-producer-date.xls").contains(&(60, "d.m".to_owned())));
}

#[test]
fn declared_dimensions_keep_exclusive_ends() {
    for (file, expected) in [
        ("legacy-range-absent.xls", None),
        ("legacy-empty-zero.xls", Some(((0, 0), (0, 0)))),
        ("legacy-empty-one.xls", Some(((0, 0), (1, 1)))),
        ("legacy-declared-offset.xls", Some(((4, 2), (20, 8)))),
        ("legacy-empty-limit.xls", Some(((0, 0), (65536, 256)))),
    ] {
        let stream = stream(file);
        let mut records = Records::new(&stream);
        let mut actual = None;
        while let Some(record) = records.next().unwrap() {
            if record.kind == 0x0200 {
                actual = Some(super::records::dimensions(&mut record.payload(), true).unwrap());
            }
        }
        assert_eq!(actual, expected);
    }
}

#[test]
fn cell_format_selection_defaults_only_numbers() {
    assert_eq!(super::format_id(&[164], 0, true), Some(164));
    assert_eq!(super::format_id(&[164], 0, false), Some(164));
    assert_eq!(super::format_id(&[164], 1, true), Some(0));
    assert_eq!(super::format_id(&[164], 1, false), None);
}

#[test]
fn string_record_requires_a_pending_formula_string() {
    let mut data = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/workbooks/legacy-formula-caches.xls"),
    )
    .unwrap();
    let stream = cfb::workbook_stream(&data).unwrap();
    let mut records = Records::new(&stream);
    let mut pending = None;
    while let Some(record) = records.next().unwrap() {
        if record.kind == 0x0006 {
            let mut payload = record.payload();
            super::CellHeader::parse(&mut payload).unwrap();
            if matches!(
                super::FormulaValue::parse(&mut payload).unwrap(),
                super::FormulaValue::PendingString
            ) {
                pending = Some(record.offset);
            }
        }
    }
    let offset = pending.unwrap();
    let needle = &stream[offset..offset + 18];
    let positions: Vec<_> = data
        .windows(needle.len())
        .enumerate()
        .filter_map(|(index, bytes)| (bytes == needle).then_some(index))
        .collect();
    assert_eq!(positions.len(), 1);
    data[positions[0] + 10] = 3;
    let error = super::read(&data, "orphan-string.xls", &super::Ssf::default()).unwrap_err();
    assert!(matches!(error, super::WorkbookError::Reader(_)));
    assert!(error.to_string().starts_with("orphan-string.xls:"));
}

thread_local! {
    /// Payload termination queries and continuation-piece inspections on this thread.
    pub(super) static PAYLOAD_WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Shared-string records with the writer's 8,224-byte limit and whole-string boundaries.
fn continued_strings(count: u32) -> Vec<u8> {
    let mut stream = Vec::new();
    let mut piece = Vec::new();
    piece.extend(count.to_le_bytes());
    piece.extend(count.to_le_bytes());
    let mut kind = 0x00fc_u16;
    for index in 0..count {
        if piece.len() + 13 > 8224 {
            stream.extend(kind.to_le_bytes());
            stream.extend(u16::try_from(piece.len()).unwrap().to_le_bytes());
            stream.append(&mut piece);
            kind = 0x003c;
        }
        piece.extend(5_u16.to_le_bytes());
        piece.push(1);
        piece.extend(
            format!("s{index:04}")
                .encode_utf16()
                .flat_map(u16::to_le_bytes),
        );
    }
    stream.extend(kind.to_le_bytes());
    stream.extend(u16::try_from(piece.len()).unwrap().to_le_bytes());
    stream.extend(piece);
    stream
}

#[test]
fn shared_string_continuations_keep_linear_termination_work() {
    for (count, pieces) in [(4096, 7), (8192, 13)] {
        let stream = continued_strings(count);
        let record = Records::new(&stream).next().unwrap().unwrap();
        PAYLOAD_WORK.set(0);
        let strings = strings::shared_strings(&record).unwrap();
        let work = PAYLOAD_WORK.get();
        assert_eq!(strings.len(), usize::try_from(count).unwrap());
        assert_eq!(strings.first().unwrap(), "s0000");
        assert_eq!(strings.last().unwrap(), &format!("s{:04}", count - 1));
        println!("strings={count} pieces={pieces} termination_work={work}");
        assert!(work <= usize::try_from(count).unwrap() + 2 * pieces);
    }
}

#[test]
fn empty_strings_consume_only_their_encoding_flag() {
    for (encoding, bytes) in [
        (Encoding::CodePage(1252), vec![0, 0, 0, 0, 0xaa]),
        (Encoding::Unicode, vec![0, 0, 1, 0, 0, 0, 0, 1, 0xaa]),
    ] {
        let mut stream = vec![0x04, 0x02];
        stream.extend(u16::try_from(bytes.len()).unwrap().to_le_bytes());
        stream.extend(bytes);
        let record = Records::new(&stream).next().unwrap().unwrap();
        let mut payload = record.payload();
        assert_eq!(strings::unicode_string(&mut payload, encoding).unwrap(), "");
        assert_eq!(strings::format_string(&mut payload, encoding).unwrap(), "");
        assert_eq!(strings::short_string(&mut payload, encoding).unwrap(), "");
        assert_eq!(payload.remaining(), 1);
        assert_eq!(payload.u8().unwrap(), 0xaa);
        assert_eq!(payload.remaining(), 0);
    }
}

thread_local! {
    /// Binary token decodes in the current workbook read.
    pub(super) static TOKEN_WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Direct template anchor lookups in the current workbook read.
    pub(super) static ANCHOR_WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Ordered array-adoption comparisons in the current workbook read.
    pub(super) static ARRAY_WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Positions visited in hyperlink rectangles.
    pub(super) static LINK_WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[test]
fn legacy_metadata_work_tracks_tokens_templates_and_link_cells() {
    for size in [32, 64] {
        for kind in ["tokens", "shared", "arrays", "arrayAnchors", "links"] {
            TOKEN_WORK.set(0);
            ANCHOR_WORK.set(0);
            ARRAY_WORK.set(0);
            LINK_WORK.set(0);
            let file = format!("metadata-work-{kind}-{size}.xls");
            let bytes = std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/workbooks")
                    .join(&file),
            )
            .unwrap();
            let book = super::read(&bytes, &file, &super::Ssf::default()).unwrap();
            let sheet = &book.sheets["Data"];
            let work = [
                TOKEN_WORK.get(),
                ANCHOR_WORK.get(),
                ARRAY_WORK.get(),
                LINK_WORK.get(),
            ];
            println!("kind={kind} size={size} work={work:?}");
            check_metadata_work(kind, size, sheet, work);
        }
    }
}

/// Compare work bounds with the corresponding retained workbook result.
fn check_metadata_work(kind: &str, size: usize, sheet: &super::WorkSheet, work: [usize; 4]) {
    match kind {
        "tokens" => {
            assert_eq!(
                sheet.cells["A1"].f.as_deref(),
                Some(format!("1{}", "+2".repeat(size)).as_str())
            );
            assert_eq!(work[0], 1 + 2 * size);
        }
        "shared" => {
            assert_eq!(sheet.cells.len(), size);
            assert!(
                sheet
                    .cells
                    .values()
                    .all(|cell| cell.f.as_deref() == Some("7"))
            );
            assert!(work[1] <= size + 1);
            assert_eq!(work[2], 0);
        }
        "arrayAnchors" => {
            assert_eq!(sheet.cells.len(), 2 * size);
            assert_eq!(work[2], 0);
            assert!(work[1] <= 3 * size);
            check_array_anchors(sheet, size);
        }
        "arrays" => {
            check_materialized_arrays(sheet, size, work[2]);
        }
        "links" => {
            assert_eq!(sheet.cells.len(), size);
            assert_eq!(work[3], 2 * size);
            assert!(
                sheet
                    .cells
                    .values()
                    .all(|cell| cell.l.as_ref().is_some_and(|link| link.target == "x"))
            );
        }
        _ => unreachable!(),
    }
}

/// Observe both array metadata owners for every direct anchor lookup.
fn check_array_anchors(sheet: &super::WorkSheet, size: usize) {
    for row in 1..=size {
        assert_eq!(
            sheet.cells[&format!("A{row}")].f.as_deref(),
            Some((row - 1).to_string().as_str())
        );
        assert_eq!(
            sheet.cells[&format!("B{row}")].array_formula.as_deref(),
            Some(format!("A{row}:B{row}").as_str())
        );
        assert!(sheet.cells[&format!("B{row}")].f.is_none());
    }
}

/// Preserve ordered array adoption only on materialized-expression paths.
fn check_materialized_arrays(sheet: &super::WorkSheet, size: usize, work: usize) {
    assert_eq!(sheet.cells.len(), 3 * size);
    assert_eq!(work, 3 * size * (size + 1) / 2);
    for row in 1..=size {
        let range = format!("A{row}:B{row}");
        let anchor = &sheet.cells[&format!("A{row}")];
        assert_eq!(anchor.array_formula.as_deref(), Some(range.as_str()));
        assert_eq!(anchor.f.as_deref(), Some((row - 1).to_string().as_str()));
        assert_eq!(anchor.v, super::CellValue::Text("v".into()));
        let follower = &sheet.cells[&format!("B{row}")];
        assert_eq!(follower.array_formula.as_deref(), Some(range.as_str()));
        assert!(follower.f.is_none());
        assert_eq!(follower.v, super::CellValue::Number(7.0));
        let neighbour = &sheet.cells[&format!("C{row}")];
        assert!(neighbour.array_formula.is_none());
        assert_eq!(neighbour.f.as_deref(), Some("1"));
        assert_eq!(neighbour.v, super::CellValue::Number(7.0));
    }
}

thread_local! {
    /// Bytes written while assembling owned expression fragments and final text.
    pub(super) static ASSEMBLY_WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[test]
fn formula_assembly_writes_linear_bytes_for_every_composition() {
    for kind in ["tokens", "sum", "unary", "function", "space"] {
        let mut previous = 0;
        for size in [32, 64] {
            let file = format!("metadata-work-{kind}-{size}.xls");
            let bytes = std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/workbooks")
                    .join(&file),
            )
            .unwrap();
            ASSEMBLY_WORK.set(0);
            let book = super::read(&bytes, &file, &super::Ssf::default()).unwrap();
            let expression = book.sheets["Data"].cells["A1"].f.as_deref().unwrap();
            let expected = match kind {
                "tokens" => format!("1{}", "+2".repeat(size)),
                "sum" => format!("{}1{}", "SUM(".repeat(size), ")".repeat(size)),
                "unary" => format!("{}1{}", "(".repeat(size), ")".repeat(size)),
                "function" => format!("{}1{}", "ABS(".repeat(size), ")".repeat(size)),
                "space" => format!("{}1{}", " (".repeat(size), ")".repeat(size)),
                _ => unreachable!(),
            };
            assert_eq!(expression, expected);
            let work = ASSEMBLY_WORK.get();
            println!(
                "assembly kind={kind} size={size} output={} bytes_written={work}",
                expression.len()
            );
            assert!(
                work <= 2 * expression.len(),
                "repeated accumulated expression copying"
            );
            if previous != 0 {
                assert!(work <= 2 * previous);
            }
            previous = work;
        }
    }
}

/// Exercise coordinate emission directly after the existing binary token decoder.
fn formula_expression(
    file: &str,
    encoding: Encoding,
    kind: u16,
    position: super::CellAddress,
) -> String {
    let stream = stream(file);
    let mut records = Records::new(&stream);
    while let Some(record) = records.next().unwrap() {
        if record.kind == kind {
            let mut payload = record.payload();
            payload.skip(if kind == 6 { 20 } else { 8 }).unwrap();
            let tokens = super::formulas::parse(&mut payload, encoding).unwrap();
            return super::formulas::stringify(
                &tokens,
                position,
                &super::formulas::SupBooks::default(),
                encoding,
            )
            .unwrap();
        }
    }
    unreachable!()
}

#[test]
fn formula_coordinate_assembly_keeps_version_and_range_context() {
    assert_eq!(
        formula_expression(
            "metadata-fix-biff5-negative.xls",
            Encoding::CodePage(1252),
            6,
            super::CellAddress { r: 0, c: 0 }
        ),
        "$A16384"
    );
    for class in [45, 77, 109] {
        assert_eq!(
            formula_expression(
                &format!("metadata-fix-shifted-area-{class}.xls"),
                Encoding::Unicode,
                6,
                super::CellAddress { r: 2, c: 2 }
            ),
            "3:4"
        );
        assert_eq!(
            formula_expression(
                &format!("metadata-fix-shared-area-{class}.xls"),
                Encoding::Unicode,
                0x4bc,
                super::CellAddress { r: 0, c: 0 }
            ),
            "SUM($1:$2)"
        );
    }
}

#[test]
fn external_sheet_layout_selection_accepts_both_payload_forms() {
    for payload in [
        vec![4, 0x44, 0x61, 0x74, 0x61],
        vec![4, 3, 0x44, 0x61, 0x74, 0x61],
        vec![1, 0, 0, 0, 0, 0, 0, 0],
    ] {
        let mut stream = vec![0x17, 0];
        stream.extend(u16::try_from(payload.len()).unwrap().to_le_bytes());
        stream.extend(payload);
        let record = Records::new(&stream).next().unwrap().unwrap();
        let mut payload = record.payload();
        let mut books = super::formulas::SupBooks::default();
        assert!(
            books
                .external_sheets(&mut payload, Encoding::Unicode)
                .is_ok()
        );
        assert_eq!(payload.remaining(), 0);
    }
}
