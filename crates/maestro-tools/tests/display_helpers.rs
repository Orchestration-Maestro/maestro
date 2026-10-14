//! Shared display operations: escape removal, binary-output cleaning, string selection,
//! path abbreviation, key hints and result text.
#![cfg(test)]

use crate::display_support;

use display_support::{Env, bind, cases};
use maestro_agent::AgentToolResult;
use maestro_models::UserBlock;
use maestro_tools::{
    key_hint, key_text,
    output::render_utils::{
        get_text_output, normalize_display_text, replace_tabs, shorten_path, str, strip_ansi,
    },
    raw_key_hint,
    utils::shell::sanitize_binary_output,
};
use maestro_tui::{TruncateOptions, truncate_to_width, visible_width};
use serde_json::{Value, json};

const STRIP: &str = "display_stripping_matches_complete_escape_sequences";

#[test]
fn display_stripping_matches_complete_escape_sequences() {
    for case in cases(STRIP) {
        let text = case.input["text"].as_str().unwrap();
        assert_eq!(
            strip_ansi(text).unwrap(),
            case.expected.as_str().unwrap(),
            "case {}",
            case.id
        );
    }
}

#[test]
fn shell_sanitization_keeps_only_allowed_controls() {
    for case in cases("shell_sanitization_keeps_only_allowed_controls") {
        let text = case.input["text"].as_str().unwrap();
        assert_eq!(
            sanitize_binary_output(text),
            case.expected.as_str().unwrap(),
            "case {}",
            case.id
        );
    }
}

#[test]
fn display_string_selection_distinguishes_absent_and_wrong_types() {
    for case in cases("display_string_selection_distinguishes_absent_and_wrong_types") {
        let expected = case.expected.as_str();
        assert_eq!(str(case.input.get("value")), expected, "case {}", case.id);
    }
}

#[test]
fn display_literal_transforms_preserve_other_whitespace() {
    for case in cases("display_literal_transforms_preserve_other_whitespace") {
        let text = case.input["text"].as_str().unwrap();
        let actual = match case.op.as_str() {
            "tabs" => replace_tabs(text),
            "cr" => normalize_display_text(text),
            other => panic!("unknown operation {other}"),
        };
        assert_eq!(actual, case.expected.as_str().unwrap(), "case {}", case.id);
    }
}

#[test]
fn display_paths_abbreviate_only_home_and_descendants() {
    let home = maestro_tools::expand_path("~").unwrap();
    for case in cases("display_paths_abbreviate_only_home_and_descendants") {
        let path = case.input["path"].as_str().unwrap().replace("$HOME", &home);
        let expected = case.expected.as_str().unwrap().replace("$HOME", &home);
        assert_eq!(
            shorten_path(Some(&path)).unwrap(),
            expected,
            "case {}",
            case.id
        );
    }
    assert_eq!(shorten_path(None).unwrap(), "");
}

#[test]
fn key_hints_use_current_bindings_order_and_separate_styles() {
    for case in cases("key_hints_use_current_bindings_order_and_separate_styles") {
        let keys: Vec<String> = serde_json::from_value(case.input["binding"].clone()).unwrap();
        let _bindings = bind(&keys);
        let env = Env::new(0);
        let actual = json!({
            "text": key_text("app.tools.expand"),
            "hint": key_hint("app.tools.expand", "to expand", &env.live).unwrap(),
            "raw": raw_key_hint("f2", "", &env.live).unwrap(),
            "missing": key_hint("missing", "help", &env.live).unwrap(),
        });
        assert_eq!(actual, case.expected, "case {}", case.id);
    }
}

#[test]
fn display_width_cases_use_shared_toolkit_boundaries() {
    for case in cases("display_width_cases_use_shared_toolkit_boundaries") {
        let width = usize::try_from(case.input["width"].as_u64().unwrap()).unwrap();
        let text = truncate_to_width(
            case.input["text"].as_str().unwrap(),
            width,
            TruncateOptions::default(),
        );
        assert_eq!(
            json!({"text": text, "width": visible_width(&text)}),
            case.expected,
            "case {}",
            case.id
        );
    }
}

#[test]
fn display_output_handles_absence_and_orders_content_by_kind() {
    let env = Env::new(0);
    for case in cases("display_output_handles_absence_and_orders_content_by_kind") {
        let result = case.input["result"]
            .as_object()
            .map(|result| AgentToolResult::<Value> {
                content: serde_json::from_value::<Vec<UserBlock>>(result["content"].clone())
                    .unwrap(),
                details: None,
                terminate: None,
            });
        let output = get_text_output(
            result
                .as_ref()
                .map(maestro_tools::output::render_utils::ToolRenderResultLike::from)
                .as_ref(),
            false,
            &env.terminal,
        )
        .unwrap();
        assert_eq!(output, case.expected.as_str().unwrap(), "case {}", case.id);
    }
    let distinct = AgentToolResult::<Value> {
        content: serde_json::from_value(json!([
            {"type": "text", "text": "b"},
            {"type": "image", "data": "", "mimeType": "image/png"},
            {"type": "text", "text": "a"},
            {"type": "image", "data": "", "mimeType": "image/gif"},
        ]))
        .unwrap(),
        details: None,
        terminate: None,
    };
    let output = get_text_output(
        Some(&maestro_tools::output::render_utils::ToolRenderResultLike::from(&distinct)),
        true,
        &env.terminal,
    )
    .unwrap();
    assert_eq!(output, "b\na\n[Image: [image/png]]\n[Image: [image/gif]]");
}

#[test]
fn borrowed_result_view_keeps_error_and_optional_details() {
    use maestro_tools::output::render_utils::ToolRenderResultLike;
    let source = maestro_agent::AgentToolResult::<serde_json::Value> {
        content: Vec::new(),
        details: Some(serde_json::Value::Null),
        terminate: None,
    };
    let mut view = ToolRenderResultLike::from(&source);
    assert!(std::ptr::eq(view.content, source.content.as_slice()));
    assert!(std::ptr::eq(
        view.details.unwrap(),
        source.details.as_ref().unwrap()
    ));
    assert!(view.details.unwrap().is_null());
    assert_eq!(view.is_error, None);
    view.is_error = Some(true);
    assert_eq!(view.is_error, Some(true));
    let absent = maestro_agent::AgentToolResult::<serde_json::Value> {
        content: Vec::new(),
        details: None,
        terminate: None,
    };
    assert!(ToolRenderResultLike::from(&absent).details.is_none());
}
