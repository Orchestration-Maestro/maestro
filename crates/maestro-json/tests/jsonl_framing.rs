//! JSONL framing through the public synchronous codec.
#![cfg(test)]

use maestro_json::jsonl::serialize_json_line;
use serde_json::json;

#[test]
fn json_line_keeps_unicode_separators() {
    let value = json!({"text": "a\u{2028}b\u{2029}c"});
    let line = serialize_json_line(&value).unwrap();
    assert_eq!(line, "{\"text\":\"a\u{2028}b\u{2029}c\"}\n");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&line).unwrap(),
        value
    );
}

#[test]
fn jsonl_only_lf_ends_a_record() {
    let mut splitter = maestro_json::jsonl::JsonlLineSplitter::default();
    let content = "a\u{2028}b\u{2029}c\u{85}\u{b}\u{c}\rd";
    assert!(splitter.push(content.as_bytes()).is_empty());
    assert_eq!(splitter.push(b"\n\nx\n"), [content, "", "x"]);
    assert_eq!(splitter.finish(), None);
}

/// One complete value and its recorded wire bytes.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Serialization {
    input: String,
    expected: String,
}

/// Byte chunks and their complete ordered decoded records, including the final tail.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Partition {
    chunks: Vec<Vec<u8>>,
    expected: Vec<String>,
}

/// The complete independent corpus.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    serialize: Vec<Serialization>,
    split: Vec<Partition>,
}

/// Read the strict fixture without dropping unknown fields.
fn fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/jsonl_framing.json")).unwrap()
}

#[test]
fn json_line_reuses_canonical_json() {
    for row in fixture().serialize {
        let value = serde_json::from_str(&row.input).unwrap();
        assert_eq!(serialize_json_line(&value).unwrap(), row.expected);
    }
}

#[test]
fn jsonl_removes_one_terminal_cr() {
    let mut splitter = maestro_json::jsonl::JsonlLineSplitter::default();
    assert!(splitter.push(b"a\r").is_empty());
    assert_eq!(splitter.push(b"\nb\r\r\n\r\n"), ["a", "b\r", ""]);
    assert!(splitter.push(b"\r").is_empty());
    assert_eq!(splitter.finish().as_deref(), Some(""));
}

#[test]
fn jsonl_retains_split_utf8() {
    let bytes = "é☃🦀\n".as_bytes();
    for cut in 0..=bytes.len() {
        let mut splitter = maestro_json::jsonl::JsonlLineSplitter::default();
        let mut lines = splitter.push(&bytes[..cut]);
        lines.extend(splitter.push(&bytes[cut..]));
        assert_eq!(lines, ["é☃🦀"]);
        assert_eq!(splitter.finish(), None);
    }
    let mut splitter = maestro_json::jsonl::JsonlLineSplitter::default();
    for byte in "é☃🦀".as_bytes() {
        assert!(splitter.push(&[*byte]).is_empty());
    }
    assert_eq!(splitter.push(b"\n"), ["é☃🦀"]);
}

#[test]
fn jsonl_flushes_unterminated_tail_once() {
    for (input, completed, tail) in [
        ("", vec![], None),
        ("tail", vec![], Some("tail")),
        ("\r", vec![], Some("")),
        ("a\n", vec!["a"], None),
    ] {
        let mut splitter = maestro_json::jsonl::JsonlLineSplitter::default();
        assert!(splitter.push(b"").is_empty());
        assert_eq!(splitter.push(input.as_bytes()), completed);
        assert_eq!(splitter.finish().as_deref(), tail);
        assert_eq!(splitter.finish(), None);
    }
}

#[test]
fn jsonl_keeps_bom_and_invalid_byte_replacements() {
    let mut splitter = maestro_json::jsonl::JsonlLineSplitter::default();
    assert_eq!(
        splitter.push(b"\xef\xbb\xbfa\n\xef\xbb\xbfb\n\xff\xc0\n\xe2\x82\n"),
        ["\u{feff}a", "\u{feff}b", "��", "�"]
    );
    assert!(splitter.push(b"\xf0\x9f").is_empty());
    assert_eq!(splitter.finish().as_deref(), Some("�"));
}

#[test]
fn jsonl_chunk_partition_preserves_lines() {
    for row in fixture().split {
        let mut splitter = maestro_json::jsonl::JsonlLineSplitter::default();
        let mut lines = Vec::new();
        for chunk in row.chunks {
            lines.extend(splitter.push(&chunk));
        }
        lines.extend(splitter.finish());
        assert_eq!(lines, row.expected);
        assert_eq!(splitter.finish(), None);
    }
    for length in [32, 64] {
        let mut splitter = maestro_json::jsonl::JsonlLineSplitter::default();
        for _ in 0..length {
            assert!(splitter.push(b"x").is_empty());
        }
        assert_eq!(splitter.finish(), Some("x".repeat(length)));
    }
}

#[test]
fn jsonl_splitters_keep_independent_state() {
    let mut left = maestro_json::jsonl::JsonlLineSplitter::default();
    let mut right = maestro_json::jsonl::JsonlLineSplitter::default();
    assert!(left.push(b"lef").is_empty());
    assert!(right.push(b"righ").is_empty());
    assert_eq!(right.push(b"t\n"), ["right"]);
    assert_eq!(left.push(b"t\nrest"), ["left"]);
    drop(right);
    assert_eq!(left.finish().as_deref(), Some("rest"));
}

#[test]
fn native_json_line_preserves_integer_boundaries() {
    let value = json!({"seq": 9_007_199_254_740_993_u64, "nested": [u64::MAX, i64::MIN],
        "float": 1e21, "zero": -0.0});
    let line = maestro_json::jsonl::serialize_native_json_line(&value).unwrap();
    assert_eq!(
        line,
        "{\"seq\":9007199254740993,\"nested\":[18446744073709551615,-9223372036854775808],\"float\":1e+21,\"zero\":0}\n"
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&line).unwrap()["seq"],
        value["seq"]
    );
}
