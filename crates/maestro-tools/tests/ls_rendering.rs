//! How the ls definition draws its calls and results.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

mod display_support;
mod ls_support;
mod read_support;

use ls_support::{cases, check_reuse, check_rows, check_theme_error};

#[test]
fn ls_call_renderer_handles_path_presence_and_invalid_arguments() {
    check_rows("ls_call_renderer_handles_path_presence_and_invalid_arguments");
}

#[test]
fn ls_call_renderer_keeps_numeric_limit_spelling() {
    check_rows("ls_call_renderer_keeps_numeric_limit_spelling");
}

#[test]
fn ls_result_renderer_trims_accepted_whitespace_and_previews_twenty_rows() {
    check_rows("ls_result_renderer_trims_accepted_whitespace_and_previews_twenty_rows");
}

#[test]
fn ls_result_renderer_combines_limit_warnings() {
    check_rows("ls_result_renderer_combines_limit_warnings");
}

#[test]
fn ls_renderers_reuse_text_without_mutating_on_errors() {
    for case in cases("ls_renderers_reuse_text_without_mutating_on_errors") {
        match case.op.as_str() {
            "reuse" => check_reuse(&case),
            _ => check_theme_error(&case),
        }
    }
}

#[test]
fn ls_renderers_use_live_keybindings_and_supplied_theme() {
    check_rows("ls_renderers_use_live_keybindings_and_supplied_theme");
}

#[test]
fn ls_result_renderer_reuses_sanitized_text_and_image_fallback() {
    check_rows("ls_result_renderer_reuses_sanitized_text_and_image_fallback");
}

#[test]
fn ls_result_renderer_uses_only_its_expansion_option() {
    check_rows("ls_result_renderer_uses_only_its_expansion_option");
}

#[test]
fn ls_result_renderer_wraps_with_shared_text_component() {
    check_rows("ls_result_renderer_wraps_with_shared_text_component");
}
