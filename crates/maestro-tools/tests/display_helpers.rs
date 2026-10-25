//! Shared display operations: escape removal, binary-output cleaning, string selection,
//! path abbreviation, key hints and result text.
#![cfg(test)]

use crate::display_cases::{cases, search_cases, search_keys};
#[cfg(not(target_arch = "wasm32"))]
use crate::display_support::{Env, Setup, bind, context};
#[cfg(not(target_arch = "wasm32"))]
use crate::read_support::isolated;
#[cfg(not(target_arch = "wasm32"))]
use maestro_theme::{Theme, ThemeColor};
use maestro_tools::output::render_utils::{normalize_display_text, replace_tabs, str, strip_ansi};
use maestro_tools::utils::shell::sanitize_binary_output;
use maestro_tui::{TruncateOptions, truncate_to_width, visible_width};
use serde_json::json;

#[cfg(not(target_arch = "wasm32"))]
use maestro_agent::AgentToolResult;
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::UserBlock;
#[cfg(not(target_arch = "wasm32"))]
use maestro_tools::{
    ReadToolOptions, create_read_tool_definition, key_hint, key_text,
    output::render_utils::{get_text_output, shorten_path},
    raw_key_hint,
};
#[cfg(not(target_arch = "wasm32"))]
use serde_json::Value;

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

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn display_paths_abbreviate_only_home_and_descendants() {
    let test = "display_paths_abbreviate_only_home_and_descendants";
    if !isolated(
        test,
        "home",
        &[
            ("HOME", Some("/home/maestro")),
            ("USERPROFILE", Some("/home/maestro")),
        ],
    ) {
        return;
    }
    let home = maestro_tools::expand_path("~").unwrap();
    for case in cases(test) {
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

/// The homes the native path display is driven with, each with a child-case name.
#[cfg(not(target_arch = "wasm32"))]
const NATIVE_HOMES: [(&str, &str); 5] = [
    ("root", "/"),
    ("empty", ""),
    ("home", "/home/user"),
    ("unicode", "/home/élève"),
    ("trailing", "/home/user/"),
];

/// The literal home and path operands of the native path display, and what each shows.
#[cfg(not(target_arch = "wasm32"))]
const SHORTENED_PATHS: [(&str, &str, &str); 15] = [
    ("/", "/tmp/supplied-home/file", "/tmp/supplied-home/file"),
    ("/", "/", "/"),
    ("/", "//server/share", "//server/share"),
    ("/", "relative/file", "relative/file"),
    ("/", "", ""),
    ("", "/tmp/supplied-home/file", "/tmp/supplied-home/file"),
    ("", "", ""),
    ("/home/user", "/home/user/file", "~/file"),
    ("/home/user", "/home/user", "~"),
    ("/home/user", "/home/username/file", "/home/username/file"),
    ("/home/user", "/tmp/file", "/tmp/file"),
    ("/home/user", "relative/file", "relative/file"),
    ("/home/user", "", ""),
    ("/home/élève", "/home/élève/文書", "~/文書"),
    ("/home/user/", "/home/user/file", "~file"),
];

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn display_paths_keep_root_and_empty_homes_unchanged() {
    let test = "display_paths_keep_root_and_empty_homes_unchanged";
    for (mode, home) in NATIVE_HOMES {
        if !isolated(
            test,
            mode,
            &[("HOME", Some(home)), ("USERPROFILE", Some(home))],
        ) {
            continue;
        }
        for (case_home, path, expected) in SHORTENED_PATHS {
            if case_home == home {
                assert_eq!(
                    shorten_path(Some(path)).unwrap(),
                    expected,
                    "{path:?} below {home:?}"
                );
            }
        }
    }
    assert_eq!(shorten_path(None).unwrap(), "");
}

/// The complete `read` call row for `args` under the current home.
#[cfg(not(target_arch = "wasm32"))]
fn read_call_row(args: &Value) -> String {
    let setup = Setup::from(&json!({"readme": "/work/README.md"}));
    let env = Env::new(setup.shift);
    let ctx = context(args, &setup, &env, None);
    let tool = create_read_tool_definition::<()>("/work".into(), ReadToolOptions::default());
    let row = (tool.render_call.as_ref().unwrap())(Some(args), &env.live, &ctx).unwrap();
    row.render(crate::display_support::WIDTH)
        .join("\n")
        .trim_end()
        .to_owned()
}

/// The complete `read` call row showing `text` in `role`.
#[cfg(not(target_arch = "wasm32"))]
fn styled_read_call(role: &ThemeColor, text: &str) -> String {
    let env = Env::new(0);
    let theme = env.live.get().unwrap();
    let title = theme
        .fg(&ThemeColor::ToolTitle, &Theme::bold("read", true))
        .unwrap();
    let display = theme.fg(role, text).unwrap();
    format!("{title} {display}")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn read_call_keeps_root_and_empty_home_paths_absolute() {
    let test = "read_call_keeps_root_and_empty_home_paths_absolute";
    for (mode, home, shown) in [
        ("root", "/", "/tmp/supplied-home/file"),
        ("empty", "", "/tmp/supplied-home/file"),
        ("home", "/tmp/supplied-home", "~/file"),
    ] {
        if !isolated(
            test,
            mode,
            &[("HOME", Some(home)), ("USERPROFILE", Some(home))],
        ) {
            continue;
        }
        let path = json!({"path": "/tmp/supplied-home/file"});
        assert_eq!(
            read_call_row(&path),
            styled_read_call(&ThemeColor::Accent, shown),
            "{mode} home"
        );
        if home == "/tmp/supplied-home" {
            continue;
        }
        for absent in [json!({}), json!({"path": null}), json!({"path": ""})] {
            assert_eq!(
                read_call_row(&absent),
                styled_read_call(&ThemeColor::ToolOutput, "..."),
                "{mode} home, {absent}"
            );
        }
        assert_eq!(
            read_call_row(&json!({"path": true})),
            styled_read_call(&ThemeColor::Error, "[invalid arg]"),
            "{mode} home, boolean path"
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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

#[test]
fn borrowed_result_view_preserves_termination_hint() {
    use maestro_tools::output::render_utils::ToolRenderResultLike;
    for terminate in [None, Some(false), Some(true)] {
        let source = maestro_agent::AgentToolResult::<serde_json::Value> {
            content: Vec::new(),
            details: None,
            terminate,
        };
        assert_eq!(ToolRenderResultLike::from(&source).terminate, terminate);
    }
}

#[test]
fn search_corpus_loading_preserves_validation_and_palette() {
    let keys = json!(["z", "a", "c", "d", "e", "f", "g"]);
    let first =
        json!({"test":"selected","id":"first","op":"call","input":{"n":2},"expected":"one"});
    let second =
        json!({"test":"selected","id":"second","op":"result","input":{"n":1},"expected":"two"});
    let other = json!({"test":"other","id":"other","op":"call","input":{},"expected":null});
    let corpus = json!({"theme_keys":keys,"cases":[first,other,second]});
    let selected = search_cases(&corpus.to_string(), "selected");
    assert_eq!(
        selected.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        ["first", "second"]
    );
    assert_eq!(selected[0].input, json!({"n":2}));
    assert_eq!(selected[0].expected, "one");
    assert_eq!(selected[1].op, "result");
    assert_eq!(selected[1].input, json!({"n":1}));
    assert_eq!(selected[1].expected, "two");
    assert_eq!(json!(search_keys(&corpus.to_string())), keys);
    // The display recording's palette is read from its own committed file on every target.
    let palette =
        crate::display_palette::palette_keys(include_str!("fixtures/read_display_palette.json"));
    assert_eq!(palette.len(), 19);
    assert_eq!(palette.first().map(String::as_str), Some("toolTitle"));
    assert_eq!(
        palette.last().map(String::as_str),
        Some("syntaxPunctuation")
    );
    let mut envelope = corpus.clone();
    envelope["unknown"] = json!(true);
    let mut case = corpus.clone();
    case["cases"][0]["unknown"] = json!(true);
    let mut duplicate = corpus.clone();
    let mut renamed = first;
    renamed["id"] = json!("renamed");
    duplicate["cases"].as_array_mut().unwrap().push(renamed);
    let mut palette = corpus.clone();
    palette["theme_keys"] = json!(["a"]);
    for source in [
        "{".into(),
        envelope.to_string(),
        case.to_string(),
        duplicate.to_string(),
        palette.to_string(),
    ] {
        assert!(std::panic::catch_unwind(|| search_cases(&source, "selected")).is_err());
    }
    assert!(std::panic::catch_unwind(|| search_cases(&corpus.to_string(), "absent")).is_err());
}
