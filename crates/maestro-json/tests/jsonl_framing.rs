//! JSONL framing through the public synchronous codec.
#![cfg(test)]

use maestro_json::jsonl::{JsonlLineSplitter, serialize_json_line};
use serde_json::json;
use std::io;

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

/// A strict walk's lines over `chunks`, ending at the first failure, as its kind and text.
fn strict(chunks: &[&[u8]], max_bytes: usize) -> Vec<Result<String, (io::ErrorKind, String)>> {
    let mut splitter = JsonlLineSplitter::default();
    let mut lines = Vec::new();
    for chunk in chunks {
        for line in splitter.push_strict(chunk, max_bytes) {
            let failed = line.is_err();
            lines.push(line.map_err(|error| (error.kind(), error.to_string())));
            if failed {
                return lines;
            }
        }
    }
    lines
}

/// `bytes` read whole, one byte at a time, and in two reads cut at every position.
fn partitions(bytes: &[u8]) -> Vec<Vec<&[u8]>> {
    let mut partitions = vec![vec![bytes], bytes.chunks(1).collect()];
    partitions.extend((1..bytes.len()).map(|cut| vec![&bytes[..cut], &bytes[cut..]]));
    partitions
}

#[test]
fn strict_lines_preserve_prior_records_before_failure() {
    let invalid = String::from_utf8(vec![0xff]).unwrap_err().to_string();
    let too_large = "line is too large".to_owned();
    for (bytes, failure) in [
        (&b"{\"a\":1}\n\xff\n{\"b\":2}\n"[..], invalid),
        (&b"{\"a\":1}\n123456789\n{\"b\":2}\n"[..], too_large.clone()),
        (&b"{\"a\":1}\n123456789"[..], too_large),
    ] {
        for partition in partitions(bytes) {
            assert_eq!(
                strict(&partition, 8),
                [
                    Ok("{\"a\":1}".to_owned()),
                    Err((io::ErrorKind::InvalidData, failure.clone()))
                ],
                "{partition:?}"
            );
        }
    }
}

#[test]
fn strict_lines_count_raw_payload_bytes() {
    const LIMIT: usize = 1024 * 1024;
    let too_large = Err((io::ErrorKind::InvalidData, "line is too large".to_owned()));
    for (payload, line) in [
        ("a".repeat(LIMIT - 1), Ok("a".repeat(LIMIT - 1))),
        ("a".repeat(LIMIT), Ok("a".repeat(LIMIT))),
        ("a".repeat(LIMIT + 1), too_large.clone()),
        (
            format!("{}\r", "a".repeat(LIMIT - 1)),
            Ok("a".repeat(LIMIT - 1)),
        ),
        (format!("{}\r", "a".repeat(LIMIT)), too_large.clone()),
        (
            format!("{}é", "a".repeat(LIMIT - 2)),
            Ok(format!("{}é", "a".repeat(LIMIT - 2))),
        ),
        (format!("{}é", "a".repeat(LIMIT - 1)), too_large.clone()),
    ] {
        assert_eq!(strict(&[payload.as_bytes(), b"\n"], LIMIT), [line]);
    }
    assert_eq!(
        strict(&["a".repeat(LIMIT + 1).as_bytes()], LIMIT),
        [too_large]
    );
    // The scalar, its CR and the LF arrive split across reads; CR fills the limit exactly.
    let line = format!("{}é\r\n", "a".repeat(LIMIT - 3));
    let bytes = line.as_bytes();
    for cut in [LIMIT - 2, LIMIT - 1, LIMIT] {
        assert_eq!(
            strict(&[&bytes[..cut], &bytes[cut..]], LIMIT),
            [Ok(format!("{}é", "a".repeat(LIMIT - 3)))]
        );
    }
}
