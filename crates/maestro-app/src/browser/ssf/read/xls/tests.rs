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
