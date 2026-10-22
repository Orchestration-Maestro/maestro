//! Note references and their page-end lists.
use super::check;

#[test]
fn note_references_number_in_page_order() {
    check("footnotes");
}

#[test]
fn disabled_footnotes_keep_references_only() {
    check("footnotes-disabled");
}

#[test]
fn endnotes_render_once_on_the_last_page() {
    check("endnotes");
}

#[test]
fn disabled_endnotes_keep_references_only() {
    check("endnotes-disabled");
}

#[test]
fn missing_note_parts_keep_reference_numbering() {
    check("notes-without-parts");
}

#[test]
fn duplicate_note_identifiers_use_the_last_note() {
    check("duplicate-note-ids");
}
