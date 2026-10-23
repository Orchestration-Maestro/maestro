//! Header and footer reference selection and placement.
use super::{check, markup, render};

#[test]
fn default_headers_and_footers_use_page_margins() {
    check("header-footer-default");
}

#[test]
fn title_page_selects_first_even_and_default_references() {
    check("title-page-and-even-odd");
}

#[test]
fn a_new_section_restarts_the_first_page_choice() {
    check("section-restarts-first-header");
}

#[test]
fn a_header_without_margins_is_not_offset() {
    check("header-without-margins");
}

#[test]
fn disabled_header_and_footer_options_skip_the_parts() {
    check("headers-disabled");
}

#[test]
fn a_missing_header_reference_renders_nothing() {
    check("missing-header-part");
}

/// The recording collapses `calc(...)` operands, so the offsets are checked exactly here.
#[test]
fn calc_offsets_keep_the_reference_operands() {
    let pages = render("header-footer-default");
    let html = markup(&pages[0]);
    assert!(
        html.starts_with("<section class=\"docx\" style=\"min-height:841.90pt;padding-bottom:108.00pt;padding-left:72.00pt;padding-right:72.00pt;padding-top:72.00pt;width:595.30pt\"><header style=\"margin-top:calc(36.00pt - 72.00pt);min-height:calc(72.00pt - 36.00pt)\">"),
        "{html}"
    );
    assert!(
        html.ends_with("<footer style=\"margin-bottom:calc(72.00pt - 108.00pt);min-height:calc(108.00pt - 72.00pt)\"><p><span>foot</span></p></footer></section>"),
        "{html}"
    );
}
