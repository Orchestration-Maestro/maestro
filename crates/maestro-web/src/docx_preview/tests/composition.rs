//! The library stylesheet, the page wrapper and the order styles are mounted in.
use super::check_preview;

#[test]
fn default_stylesheet_and_wrapper_enclose_the_pages() {
    check_preview("wrapper-default");
}

#[test]
fn unwrapped_pages_follow_the_library_styles() {
    check_preview("wrapper-off");
}

#[test]
fn print_option_hides_only_the_wrapper_rules() {
    check_preview("wrapper-hidden-on-print");
}

#[test]
fn class_namespace_reaches_every_rule_and_class() {
    check_preview("class-namespace");
}

#[test]
fn separate_style_container_receives_every_style() {
    check_preview("separate-style-container");
    check_preview("full-composition-separate");
}

#[test]
fn style_elements_follow_the_reference_order() {
    check_preview("full-composition");
}
