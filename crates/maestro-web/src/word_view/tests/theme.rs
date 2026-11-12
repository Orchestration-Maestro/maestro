//! Theme colours and fonts as document variables.
use super::check_preview;

#[test]
fn theme_colors_and_fonts_become_document_variables() {
    check_preview("theme-colors-fonts");
}

#[test]
fn missing_theme_values_keep_their_spelling() {
    check_preview("theme-partial");
    check_preview("theme-fonts-only");
}

#[test]
fn theme_values_cannot_leave_their_declaration() {
    check_preview("theme-value-escape");
}
