//! Paragraphs, runs and inline content.
use super::check;

#[test]
fn inline_runs_keep_text_breaks_tabs_and_symbols() {
    check("inline-basics");
    check("hyperlinks-and-smart-tags");
}

#[test]
fn paragraph_and_run_common_properties_override_declarations() {
    check("paragraph-and-run-common-properties");
}

#[test]
fn paragraph_styles_supply_numbering_classes() {
    check("paragraph-style-numbering");
}
