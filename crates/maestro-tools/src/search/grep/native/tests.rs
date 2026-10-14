//! Controlled fragments at the native descriptor consumers.
use super::*;

#[test]
fn stdout_records_wait_for_complete_json_and_utf8() {
    let mut records = Records::default();
    let reached = Cancellation::new();
    let row = "{\"type\":\"match\",\"data\":{\"path\":{\"text\":\"file.txt\"},\"line_number\":1,\"lines\":{\"text\":\"é\"}}}".as_bytes();
    let split = row.iter().position(|byte| *byte == 0xc3).unwrap() + 1;
    for (fragment, count) in [
        (&row[..split], 0),
        (&row[split..], 0),
        (b"\r\n".as_slice(), 1),
        (b"\nmalformed\n".as_slice(), 1),
        (row, 1),
    ] {
        records.feed(fragment, f64::INFINITY, &reached);
        assert_eq!(records.count, count);
        assert_eq!(records.rows.len(), count);
    }
    let tail = std::mem::take(&mut records.tail);
    records.record(&tail, f64::INFINITY, &reached);
    assert_eq!(records.count, 2);
    assert_eq!(records.rows.len(), 2);
    for record in records.rows {
        assert_eq!(record.path, PathBuf::from("file.txt"));
        assert_eq!(record.line, 1);
        assert_eq!(record.text.as_deref(), Some("é"));
    }
}

#[test]
fn stderr_fragments_decode_before_diagnostic_trimming() {
    let mut decoder = Utf8Stream::default();
    let first = decoder.push(b"\xef\xbb\xbf d\xc3");
    assert_eq!(first, "\u{feff} d");
    let second = decoder.push(b"\xa9nied \r\n");
    assert_eq!(second, "énied \r\n");
    let final_text = decoder.finish();
    assert_eq!(final_text, "");
    let diagnostic = first + &second + &final_text;
    assert_eq!(diagnostic.trim_matches(is_whitespace_scalar), "dénied");
}
