//! File-search rendering through the public definition callbacks.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::find_support;

#[test]
fn find_call_renderer_handles_argument_presence_and_styles() {
    find_support::check_render("find_call_renderer_handles_argument_presence_and_styles");
}

#[test]
fn find_call_renderer_preserves_limit_spelling() {
    find_support::check_render("find_call_renderer_preserves_limit_spelling");
}

#[test]
fn find_preview_trims_then_selects_twenty_rows() {
    find_support::check_render("find_preview_trims_then_selects_twenty_rows");
}

#[test]
fn find_renderer_combines_count_and_byte_warnings() {
    find_support::check_render("find_renderer_combines_count_and_byte_warnings");
}

#[test]
fn find_renderers_reuse_text_atomically() {
    find_support::check_render("find_renderers_reuse_text_atomically");
}

#[test]
fn find_renderers_use_live_keybindings_and_supplied_theme() {
    find_support::check_render("find_renderers_use_live_keybindings_and_supplied_theme");
}

#[test]
fn find_renderer_reuses_sanitized_text_and_image_fallback() {
    find_support::check_render("find_renderer_reuses_sanitized_text_and_image_fallback");
}

#[test]
fn find_renderer_expansion_comes_only_from_options() {
    find_support::check_render("find_renderer_expansion_comes_only_from_options");
}

#[test]
fn find_renderer_wraps_using_the_shared_text_component() {
    find_support::check_render("find_renderer_wraps_using_the_shared_text_component");
}
