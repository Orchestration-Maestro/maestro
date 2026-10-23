//! Embedded font faces.
use super::check_preview;

#[test]
fn embedded_fonts_become_font_faces_with_decoded_bytes() {
    check_preview("embedded-fonts");
    check_preview("embedded-fonts-data-urls");
}

#[test]
fn ignored_fonts_emit_no_font_faces() {
    check_preview("embedded-fonts-ignored");
}

#[test]
fn font_names_stay_inside_the_font_face_rule() {
    check_preview("font-name-escape");
}
