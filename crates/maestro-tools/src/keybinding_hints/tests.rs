//! The display spelling of key ids on each platform.

use super::format_key_text_for;

/// `alt` is shown as `option` where the platform names it so, and nowhere else.
#[test]
fn alt_is_shown_under_the_platform_name() {
    assert_eq!(
        format_key_text_for("alt+enter", false, "option"),
        "option+enter"
    );
    assert_eq!(
        format_key_text_for("Alt+Enter", false, "option"),
        "option+Enter"
    );
    assert_eq!(format_key_text_for("alt+enter", false, "alt"), "alt+enter");
    assert_eq!(
        format_key_text_for("shift+alt+x", false, "option"),
        "shift+option+x"
    );
}

/// A display spelling capitalizes the first letter of each part of each alternative.
#[test]
fn display_capitalizes_each_part_of_each_alternative() {
    assert_eq!(
        format_key_text_for("ctrl+alt+x/alt+up", true, "option"),
        "Ctrl+Option+X/Option+Up"
    );
    assert_eq!(
        format_key_text_for("shift+x/ctrl+q", true, "alt"),
        "Shift+X/Ctrl+Q"
    );
    assert_eq!(format_key_text_for("", true, "alt"), "");
}

/// A part without `alt` is untouched, including `alt`-like words and non-ASCII keys.
#[test]
fn other_parts_keep_their_spelling() {
    assert_eq!(
        format_key_text_for("altgr+\u{e9}", false, "option"),
        "altgr+\u{e9}"
    );
    assert_eq!(
        format_key_text_for("altgr+\u{e9}", true, "option"),
        "Altgr+\u{c9}"
    );
}
