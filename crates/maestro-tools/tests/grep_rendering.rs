//! Content-search terminal callbacks.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

#[test]
fn grep_call_renders_partial_arguments_and_glob() {
    crate::grep_support::check_rows("grep_call_renders_partial_arguments_and_glob");
}

#[test]
fn grep_call_retains_present_limit_labels() {
    crate::grep_support::check_rows("grep_call_retains_present_limit_labels");
}

#[test]
fn grep_preview_selects_fifteen_rows() {
    crate::grep_support::check_rows("grep_preview_selects_fifteen_rows");
}

#[test]
fn grep_preview_uses_shared_whitespace_and_sanitization() {
    crate::grep_support::check_rows("grep_preview_uses_shared_whitespace_and_sanitization");
}

#[test]
fn grep_renderer_combines_all_warning_kinds() {
    crate::grep_support::check_rows("grep_renderer_combines_all_warning_kinds");
}

#[test]
fn grep_renderers_use_live_keybindings_and_themes() {
    crate::grep_support::check_rows("grep_renderers_use_live_keybindings_and_themes");
}

#[test]
fn grep_only_result_options_control_expansion() {
    crate::grep_support::check_rows("grep_only_result_options_control_expansion");
}

#[test]
fn grep_preview_wraps_with_shared_text() {
    crate::grep_support::check_rows("grep_preview_wraps_with_shared_text");
}

#[test]
fn grep_renderers_reuse_text_without_partial_mutation() {
    use crate::{grep_support as g, ls_support};
    let result =
        g::rendered_result(&serde_json::json!({"content":[{"type":"text","text":"after"}]}));
    let args = serde_json::json!({"pattern":"after"});
    for case in g::cases("grep_renderers_reuse_text_without_partial_mutation") {
        match case.op.as_str() {
            "reuse" => ls_support::check_reuse_as(&case, &g::keys(), g::tool(), &result, &args),
            "theme-error" => {
                ls_support::check_theme_error_as(&case, &g::keys(), g::tool(), &result, &args);
            }
            _ => panic!("unrecognized case"),
        }
    }
}
