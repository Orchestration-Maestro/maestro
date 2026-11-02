//! Result visibility, first-text selection and native error observations.
use super::{expected, fg, loaded, run, theme};
use maestro_extensions::{ExtensionRunner, Local};
use maestro_test_directory::Scratch;
use serde_json::{Value, json};
use std::rc::Rc;

/// Paints a caller-supplied result through the registered renderer.
fn result_rows(
    runner: &ExtensionRunner<Local>,
    name: &str,
    content: &Value,
    flags: [bool; 2],
) -> Vec<String> {
    let [expanded, is_error] = flags;
    let tool = runner
        .get_all_registered_tools()
        .into_iter()
        .find(|tool| tool.definition.name() == name)
        .unwrap();
    let content: Vec<_> = serde_json::from_value(content.clone()).unwrap();
    let result = maestro_extensions::ToolRenderResultLike {
        content: &content,
        details: None,
        terminate: None,
        is_error: Some(is_error),
    };
    let rows = maestro_extensions::ToolRendererState::default();
    let redraws = Rc::new(std::cell::Cell::new(0));
    let args = json!({});
    let rendering = super::super::canonical_ui::row_context(&args, &rows, &redraws);
    tool.definition
        .renderer_binding()
        .unwrap()
        .render_result
        .unwrap()
        .prepare(result)
        .unwrap()
        .render(
            maestro_extensions::ToolRenderResultOptions {
                expanded,
                is_partial: false,
            },
            &theme(),
            &rendering,
        )
        .unwrap()
        .try_render(200)
        .unwrap()
}

/// Checks collapsed counts and the first expanded text, including a preceding image.
fn assert_result_states(runner: &ExtensionRunner<Local>, name: &str, noun: &str) {
    use maestro_theme::ThemeColor;
    let output = |s| fg(&ThemeColor::ToolOutput, s);
    let content = json!([{"type":"image","data":"AA==","mimeType":"image/png"},{"type":"text","text":" one\ntwo \n"},{"type":"text","text":"ignored"}]);
    let collapsed = if noun.is_empty() {
        String::new()
    } else {
        fg(&ThemeColor::Muted, &format!(" → 2 {noun}"))
    };
    assert_eq!(
        result_rows(runner, name, &content, [false; 2]),
        expected(collapsed),
        "{name} collapsed"
    );
    let expanded = match name {
        "read" => format!("\n{}\n{}\n{}", output(" one"), output("two "), output("")),
        "write" | "edit" => format!("\n{}", output(" one\ntwo \n")),
        _ => format!("\n{}\n{}", output("one"), output("two")),
    };
    assert_eq!(
        result_rows(runner, name, &content, [true, false]),
        expected(expanded),
        "{name} expanded"
    );
}

/// Checks missing text, an image without text, and the tool-specific empty expansion.
fn assert_empty_results(runner: &ExtensionRunner<Local>, name: &str) {
    for content in [
        json!([]),
        json!([{"type":"image","data":"AA==","mimeType":"image/png"}]),
    ] {
        for expanded in [false, true] {
            assert_eq!(
                result_rows(runner, name, &content, [expanded, false]),
                expected(String::new()),
                "{name} missing text"
            );
        }
    }
    let blank = json!([{"type":"text","text":""}]);
    assert_eq!(
        result_rows(runner, name, &blank, [false; 2]),
        expected(String::new()),
        "{name} empty collapsed"
    );
    let expanded = if matches!(name, "bash" | "write") {
        String::new()
    } else {
        format!("\n{}", fg(&maestro_theme::ThemeColor::ToolOutput, ""))
    };
    assert_eq!(
        result_rows(runner, name, &blank, [true, false]),
        expected(expanded),
        "{name} empty expanded"
    );
}

#[test]
fn minimal_mode_collapses_results_and_expands_the_first_text_block() {
    run(async {
        let root = Scratch::new().unwrap();
        let runner = loaded(root.path()).await;
        for (name, noun) in [
            ("read", ""),
            ("bash", ""),
            ("write", ""),
            ("edit", ""),
            ("find", "files"),
            ("grep", "matches"),
            ("ls", "entries"),
        ] {
            assert_result_states(&runner, name, noun);
            assert_empty_results(&runner, name);
        }
        assert_eq!(
            result_rows(
                &runner,
                "write",
                &json!([{"type":"text","text":""},{"type":"text","text":"later"}]),
                [true, false]
            ),
            expected(String::new())
        );
        assert_eq!(
            result_rows(
                &runner,
                "bash",
                &json!([{"type":"text","text":" \n\t\u{feff}"}]),
                [true, false]
            ),
            expected(String::new())
        );
    });
}

#[test]
fn minimal_mode_colors_write_results_from_the_error_observation() {
    run(async {
        let root = Scratch::new().unwrap();
        let runner = loaded(root.path()).await;
        for (is_error, message, role) in [
            (
                false,
                "Successfully wrote 5 bytes",
                maestro_theme::ThemeColor::ToolOutput,
            ),
            (
                true,
                "Cannot open destination",
                maestro_theme::ThemeColor::Error,
            ),
        ] {
            let text = fg(&role, message);
            assert_eq!(
                result_rows(
                    &runner,
                    "write",
                    &json!([{"type":"text","text":message}]),
                    [true, is_error]
                ),
                expected(format!("\n{text}"))
            );
        }
    });
}

#[test]
fn minimal_mode_colors_edit_results_from_the_error_observation() {
    run(async {
        let root = Scratch::new().unwrap();
        let runner = loaded(root.path()).await;
        for (is_error, message, role) in [
            (
                false,
                "Replaced Error and error labels",
                maestro_theme::ThemeColor::ToolOutput,
            ),
            (
                true,
                "Cannot open destination",
                maestro_theme::ThemeColor::Error,
            ),
        ] {
            let text = fg(&role, message);
            assert_eq!(
                result_rows(
                    &runner,
                    "edit",
                    &json!([{"type":"text","text":message}]),
                    [true, is_error]
                ),
                expected(format!("\n{text}"))
            );
        }
    });
}
