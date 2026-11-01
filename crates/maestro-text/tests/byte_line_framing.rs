//! Raw line separators retain bytes across arbitrary chunk boundaries.
use maestro_text::line_decoder::ByteLineDecoder;

#[test]
fn byte_line_framing_handles_every_split_of_cr_lf_and_crlf() {
    let bytes = "a\r\nb\nc\rd\r\n\r\n雪".as_bytes();
    let expected: Vec<Vec<u8>> = ["a", "b", "c", "d", "", "雪"]
        .map(|line| line.as_bytes().to_vec())
        .into();
    for split in 0..=bytes.len() {
        let mut decoder = ByteLineDecoder::default();
        let mut lines = decoder.decode(&bytes[..split]);
        lines.extend(decoder.decode(&bytes[split..]));
        lines.extend(decoder.flush());
        assert_eq!(lines, expected, "split at {split}");
        assert!(decoder.flush().is_empty());
    }
}

#[test]
fn byte_line_framing_emits_cr_immediately_and_skips_only_one_lf() {
    let mut decoder = ByteLineDecoder::default();
    assert_eq!(decoder.decode(b"first\r"), vec![b"first".to_vec()]);
    assert!(decoder.decode(b"").is_empty());
    assert!(decoder.decode(b"\n").is_empty());
    assert_eq!(decoder.decode(b"\n"), vec![Vec::<u8>::new()]);
    assert!(decoder.flush().is_empty());
}

#[test]
fn byte_line_framing_preserves_bom_and_invalid_utf8_for_the_caller() {
    let mut decoder = ByteLineDecoder::default();
    assert!(decoder.decode(&[0xef, 0xbb]).is_empty());
    assert_eq!(
        decoder.decode(&[0xbf, 0xff, b'\n']),
        vec![vec![0xef, 0xbb, 0xbf, 0xff]]
    );
}
