//! Incremental Unicode delivery through the public stream.

use maestro_terminal::stream::{Action, Handler, Stream};

#[derive(Default)]
struct Text(String);

impl Handler for Text {
    fn vt(&mut self, action: Action) {
        if let Action::Print(cp) = action {
            self.0.push(cp);
        }
    }
}

#[test]
fn escape_flushes_an_incomplete_scalar_once() {
    for prefix in [&b"\xc2"[..], &b"\xe2\x82"[..], &b"\xf0\x9f\x98"[..]] {
        let mut stream = Stream::new(false);
        let mut text = Text::default();
        stream.next_slice(prefix, &mut text);
        assert!(text.0.is_empty());
        stream.next_slice(b"\x1bD\x80x", &mut text);
        assert_eq!(text.0, "\u{fffd}\u{fffd}x");
    }
}

fn decode(bytes: &[u8], expected: &str) {
    for split in 0..=bytes.len() {
        let mut stream = Stream::new(false);
        let mut text = Text::default();
        stream.next_slice(&bytes[..split], &mut text);
        stream.next_slice(&[], &mut text);
        stream.next_slice(&bytes[split..], &mut text);
        assert_eq!(text.0, expected, "split {split}");
    }
    let mut stream = Stream::new(false);
    let mut text = Text::default();
    for &byte in bytes {
        stream.next(byte, &mut text);
    }
    assert_eq!(text.0, expected);
}

#[test]
fn byte_run_decode_no_escape() {
    let bytes = "hello".repeat(128);
    decode(bytes.as_bytes(), &bytes);
}

#[test]
fn byte_run_decode_ascii_to_escape() {
    let prefix = "hello".repeat(64);
    decode(format!("{prefix}\x1bD").as_bytes(), &prefix);
}

#[test]
fn byte_run_decode_immediate_esc_sequence() {
    decode(b"\x1b[?5sx", "x");
}

#[test]
fn byte_run_decode_incomplete_utf_8() {
    decode(b"hello\xc2", "hello");
    decode(b"hello\xe0\0", "hello�");
    decode(b"hello\xf0\x90", "hello");
}

#[test]
fn byte_run_decode_invalid_utf_8() {
    decode(b"hello\xc2\x01", "hello�");
}

#[test]
fn byte_run_decode_invalid_leading_byte_is_replaced() {
    decode(b"hello\xff", "hello�");
}

#[test]
fn byte_run_decode_invalid_continuation_in_3_byte_sequence() {
    decode(b"hello\xe2\x28world", "hello�(world");
}

#[test]
fn byte_run_decode_invalid_continuation_in_4_byte_sequence() {
    decode(b"hello\xf0\x90\x28world", "hello�(world");
}

#[test]
fn byte_run_decode_multiple_consecutive_invalid_bytes() {
    decode(b"a\x80\x80b", "a��b");
    decode(b"a\xc0\xc0b", "a��b");
}

#[test]
fn byte_run_decode_unexpected_continuation_byte_as_lead() {
    decode(b"a\x80b", "a�b");
}

#[test]
fn byte_run_decode_overlong_2_byte_encoding() {
    decode(b"a\xc0\xafb", "a��b");
}

#[test]
fn byte_run_decode_surrogate_half() {
    decode(b"a\xed\xa0\x80b", "a���b");
}

#[test]
fn byte_run_decode_valid_multibyte_surrounded_by_invalid() {
    decode(b"\x80\xc3\xa9\x80", "�é�");
}

#[test]
fn byte_run_decode_partial_utf_8_before_escape() {
    decode(b"hi\xc2\x1b[0m", "hi�");
    decode(b"\xe0\xa0\x1bX", "�");
}

#[test]
fn byte_run_decode_invalid_byte_before_escape() {
    decode(b"hi\x80\x1b[0m", "hi�");
}

#[test]
fn byte_run_table_3_8_non_shortest_form_sequences() {
    decode(b"\xc0\xaf\xe0\x80\xbf\xf0\x81\x82A", "��������A");
}

#[test]
fn byte_run_table_3_9_surrogate_sequences() {
    decode(b"\xed\xa0\x80\xed\xbf\xbf\xed\xafA", "��������A");
}

#[test]
fn byte_run_table_3_10_other_ill_formed_sequences() {
    decode(b"\xf4\x91\x92\x93\xffA\x80\xbfB", "�����A��B");
}

#[test]
fn byte_run_table_3_11_truncated_sequences() {
    decode(b"\xe1\x80\xe2\xf0\x91\x92\xf1\xbfA", "����A");
}

#[test]
fn byte_decode_ascii() {
    decode(b"Hello, World!", "Hello, World!");
}

#[test]
fn byte_decode_well_formed_utf_8() {
    decode("😄✤ÁA".as_bytes(), "😄✤ÁA");
}

#[test]
fn byte_decode_partially_invalid_utf_8() {
    decode(b"\xf0\x9f\xf0\x9f\x98\x84\xed\xa0\x80", "�😄���");
}
