//! Streaming byte decoding through the shared text boundary.
use maestro_text::Utf8Stream;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Case {
    chunks_hex: Vec<String>,
    chunks: Vec<String>,
    #[serde(rename = "final")]
    final_text: String,
}

#[test]
fn utf8_stream_keeps_bom_and_replaces_invalid_subparts() {
    let cases: Vec<Case> = serde_json::from_str(include_str!("fixtures/utf8_stream.json")).unwrap();
    for case in cases {
        let mut decoder = Utf8Stream::default();
        assert_eq!(case.chunks_hex.len(), case.chunks.len());
        for (hex, expected) in case.chunks_hex.iter().zip(case.chunks) {
            let bytes: Vec<_> = hex
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            assert_eq!(decoder.push(&bytes), expected);
        }
        assert_eq!(decoder.finish(), case.final_text);
    }
}

#[test]
fn utf8_stream_finishes_an_incomplete_suffix_once() {
    let mut decoder = Utf8Stream::default();
    assert_eq!(decoder.finish(), "");
    assert_eq!(decoder.push(&[0xf0, 0x90, 0x8d]), "");
    assert_eq!(decoder.finish(), "\u{fffd}");
    assert_eq!(decoder.finish(), "");
    assert_eq!(decoder.push(b"next"), "next");
}
