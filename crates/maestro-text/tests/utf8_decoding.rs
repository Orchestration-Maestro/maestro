//! Shared byte decoding through the text boundary.
#![cfg(test)]
use maestro_text::{decode_utf8, lossy_string};

#[test]
fn utf8_decoding_strips_one_bom_and_replaces_invalid_bytes() {
    for (bytes, expected) in [
        (&[][..], ""),
        (&[239, 187, 191][..], ""),
        (&[239, 187, 191, 239, 187, 191, 65][..], "\u{feff}A"),
        (&[65, 239, 187, 191, 66][..], "A\u{feff}B"),
        (&[0, 65, 13, 10, 9][..], "\0A\r\n\t"),
        (&[133][..], "�"),
        (&[254][..], "�"),
        (&[192, 175][..], "��"),
        (&[226, 130][..], "�"),
        (&[226, 40, 161][..], "�(�"),
        (&[237, 160, 128][..], "���"),
        (&[244, 144, 128, 128][..], "����"),
        (
            &[194, 133, 239, 187, 191, 32, 104, 101, 108, 108, 111, 32, 10][..],
            "\u{85}\u{feff} hello \n",
        ),
        (&[240, 159, 152, 128, 32, 101, 204, 129][..], "😀 e\u{301}"),
    ] {
        assert_eq!(decode_utf8(bytes), expected);
    }
}

#[test]
fn lossy_strings_keep_valid_buffers_and_replace_invalid_bytes() {
    let valid = b"\xEF\xBB\xBFa\xF0\x9F\x98\x80".to_vec();
    let address = valid.as_ptr();
    let text = lossy_string(valid);
    assert_eq!(
        (text.as_str(), text.as_ptr()),
        ("\u{feff}a\u{1f600}", address)
    );
    assert_eq!(
        lossy_string(vec![65, 192, 175, 226, 130]),
        "A\u{fffd}\u{fffd}\u{fffd}"
    );
}
