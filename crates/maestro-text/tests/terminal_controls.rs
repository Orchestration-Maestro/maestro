//! Terminal control cleaning for text a model wrote.
#![cfg(test)]
use maestro_text::terminal::clean_controls;

#[test]
fn control_sequences_leave_only_the_words_around_them() {
    let written = concat!(
        "before \x1b]52;c;aGk=\x07clip \x1b]0;title\x1b\\title ",
        "\x1b[2Jclear \x1b]8;;https://evil.example\x1b\\safe\x1b]8;;\x1b\\ ",
        "bell\x07 back\x08space\tkeep\nlines\r\nafter"
    );
    assert_eq!(
        clean_controls(written),
        "before clip title clear safe bell backspace\tkeep\nlines\r\nafter"
    );
}

#[test]
fn text_without_controls_is_unchanged() {
    let plain = "plain text, Ω ünïcode,\ttabs and\nlines\n";
    assert_eq!(clean_controls(plain), plain);
}
