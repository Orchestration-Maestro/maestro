//! How the write definition draws its calls and results.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use crate::display_support;
use crate::read_support;
use crate::write_support;

use display_support::{
    Env, Probe, Setup, WIDTH, bind, context, native_highlighter, padded, palette_of, unstyled_blank,
};
use maestro_agent::AgentToolResult;
use maestro_models::{ImageContent, TextContent, UserBlock};
use maestro_theme::{SyntaxHighlighter, ThemeColor};
use maestro_tools::{
    ToolDefinition, ToolRenderResultOptions, WriteOperations, WriteToolInput, WriteToolOptions,
    create_write_tool_definition, key_hint, output::render_utils::strip_ansi,
};
use maestro_tui::{Component, Container, Text, tui::ComponentHandle};
use read_support::{Fixture, Scratch};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{any::Any, rc::Rc, sync::Arc};
use write_support::{Journal, ScriptedWrite, rows, single_expected, theme_keys};

/// The write definition under test.
type Tool = ToolDefinition<(), WriteToolInput, ()>;

/// The definition whose renderers are under test.
fn tool() -> Tool {
    create_write_tool_definition("/work".into(), WriteToolOptions::default())
}

/// A render setup with the expand key bound to ctrl+o and the arguments still streaming.
fn setup(expanded: bool) -> Setup {
    Setup::from(&json!({
        "readme": "/opt/maestro/README.md",
        "bindings": ["ctrl+o"],
        "expanded": expanded,
    }))
}

/// `setup` once the arguments are complete.
fn settled(mut setup: Setup) -> Setup {
    setup.phase.args_complete = true;
    setup
}

/// The environment with the corpus palette and the native highlighter.
fn env() -> Env {
    Env::for_keys(&theme_keys(), 0)
}

/// Draw a call, reusing `last`.
fn draw_call(
    args: &Value,
    setup: &Setup,
    env: &Env,
    last: Option<ComponentHandle>,
) -> ComponentHandle {
    let ctx = context(args, setup, env, last);
    (tool().render_call.unwrap())(Some(args), &env.live, &ctx).unwrap()
}

/// Draw a result, reusing `last`.
fn draw_result(
    result: &AgentToolResult<()>,
    setup: &Setup,
    env: &Env,
    last: Option<ComponentHandle>,
) -> ComponentHandle {
    let ctx = context(&Value::Null, setup, env, last);
    let options = ToolRenderResultOptions {
        expanded: false,
        is_partial: false,
    };
    crate::display_support::native(
        &(tool().render_result.unwrap()),
        &maestro_tools::output::render_utils::ToolRenderResultLike::from(result),
        options,
        &env.live,
        &ctx,
    )
    .unwrap()
}

/// The rows the toolkit lays out for `text`.
fn laid_out(text: &str) -> Vec<String> {
    Text::new(text.to_owned(), 0, 0, None).render(WIDTH)
}

/// `rows` without style escapes.
fn plain(rows: &[String]) -> Vec<String> {
    rows.iter().map(|row| strip_ansi(row).unwrap()).collect()
}

/// The recorded input of a call row.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CallInput {
    /// Arguments of the call.
    args: Value,
    /// Whether the row is expanded.
    expanded: Option<bool>,
    /// Keys bound to the expand action.
    binding: Option<Vec<String>>,
}

/// Draw every recorded call of `test` and compare its rows with the recorded text laid out
/// by the toolkit, with style escapes removed when `plain_rows`.
fn check_calls(test: &str, plain_rows: bool) {
    for row in rows(test) {
        let input: CallInput = serde_json::from_value(row.input).unwrap();
        let _bindings = bind(&input.binding.unwrap_or_else(|| vec!["ctrl+o".into()]));
        let handle = draw_call(
            &input.args,
            &setup(input.expanded.unwrap_or(false)),
            &env(),
            None,
        );
        let expected = laid_out(row.expected.as_str().unwrap());
        let actual = handle.render(WIDTH);
        if plain_rows {
            assert_eq!(plain(&actual), plain(&expected), "args {}", input.args);
        } else {
            assert_eq!(actual, expected, "args {}", input.args);
        }
    }
}

#[test]
fn write_titles_preserve_display_alias_precedence() {
    check_calls("write_titles_preserve_display_alias_precedence", false);
}

#[test]
fn write_content_preview_preserves_display_whitespace() {
    check_calls("write_content_preview_preserves_display_whitespace", false);
}

#[test]
fn write_preview_limits_count_before_terminal_wrapping() {
    check_calls("write_preview_limits_count_before_terminal_wrapping", false);
    let _bindings = bind(&["ctrl+o".to_owned()]);
    let long = "x".repeat(2500);
    let args = json!({"path": "wrapped.txt", "content": format!("{long}\nsecond")});
    let handle = draw_call(&args, &setup(false), &env(), None);
    let wide = handle.render(80);
    assert!(wide.len() > 10);
    assert!(wide.iter().all(|row| !row.contains("more lines")));
}

#[test]
fn write_preview_uses_live_expand_keybindings() {
    check_calls("write_preview_uses_live_expand_keybindings", false);
}

#[test]
fn write_highlighting_follows_explicit_selected_path() {
    check_calls("write_highlighting_follows_explicit_selected_path", true);
    for (args, language) in [
        (json!({"path": "code.rs", "content": "a\tb"}), Some("rust")),
        (json!({"path": "CODE.RS", "content": "a\tb"}), Some("rust")),
        (
            json!({"path": "other.txt", "file_path": "chosen.rs", "content": "a\tb"}),
            Some("rust"),
        ),
        (json!({"path": "code.unknown", "content": "a\tb"}), None),
        (json!({"path": "LICENSE", "content": "a\tb"}), None),
    ] {
        let probe = Probe::new(true);
        let env = Env::with(
            Rc::new(palette_of(&theme_keys(), 0)),
            Rc::clone(&probe) as Rc<dyn SyntaxHighlighter>,
        );
        draw_call(&args, &settled(setup(true)), &env, None);
        let expected: Vec<(String, String)> = language
            .map(|language| ("a   b".to_owned(), language.to_owned()))
            .into_iter()
            .collect();
        assert_eq!(*probe.calls.borrow(), expected, "{args}");
    }
    let probe = Probe::new(true);
    let env = Env::with(
        Rc::new(palette_of(&theme_keys(), 0)),
        Rc::clone(&probe) as Rc<dyn SyntaxHighlighter>,
    );
    let alias =
        |content: &str| json!({"path": "other.txt", "file_path": "chosen.rs", "content": content});
    let first = draw_call(&alias("a"), &setup(true), &env, None);
    probe.calls.borrow_mut().clear();
    draw_call(&alias("a\tb"), &setup(true), &env, Some(first));
    let appended = ("a   b".to_owned(), "rust".to_owned());
    assert_eq!(*probe.calls.borrow(), [appended.clone(), appended]);
}

/// A result of the given blocks.
fn result_of(content: Vec<UserBlock>) -> AgentToolResult<()> {
    AgentToolResult {
        content,
        details: None,
        terminate: None,
        usage: None,
    }
}

/// A text block.
fn text_block(text: &str) -> UserBlock {
    UserBlock::Text(TextContent::new(text.into()))
}

/// The blocks of a recorded result.
fn recorded_blocks(blocks: &Value) -> Vec<UserBlock> {
    blocks
        .as_array()
        .unwrap()
        .iter()
        .map(|block| match block["type"].as_str().unwrap() {
            "text" => text_block(block["text"].as_str().unwrap()),
            "image" => UserBlock::Image(ImageContent {
                data: block["data"].as_str().unwrap().into(),
                mime_type: block
                    .get("mimeType")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .into(),
            }),
            other => panic!("unknown block type {other}"),
        })
        .collect()
}

#[test]
fn write_results_render_only_nonempty_errors() {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Recorded {
        #[serde(rename = "isError", default)]
        is_error: bool,
        content: Value,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        result: Recorded,
    }
    let env = env();
    for row in rows("write_results_render_only_nonempty_errors") {
        let recorded = serde_json::from_value::<Input>(row.input).unwrap().result;
        let mut setup = setup(false);
        setup.is_error = recorded.is_error;
        let result = result_of(recorded_blocks(&recorded.content));
        let handle = draw_result(&result, &setup, &env, None);
        let any: Rc<dyn Any> = Rc::<dyn Component>::clone(&handle);
        match row.expected.as_str() {
            None => {
                assert!(any.downcast::<Container>().is_ok());
                assert!(handle.render(WIDTH).is_empty());
            }
            Some(text) => {
                assert!(any.downcast::<Text>().is_ok());
                assert_eq!(handle.render(WIDTH), laid_out(text));
            }
        }
    }
}

#[test]
fn write_streaming_cache_rebuilds_or_appends_at_owned_boundaries() {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        steps: Vec<(String, Value, bool)>,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Operand {
        code: String,
        lang: String,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Step {
        same: bool,
        highlight_inputs: Vec<Operand>,
        lines: Vec<String>,
    }
    let _bindings = bind(&["ctrl+o".to_owned()]);
    for row in rows("write_streaming_cache_rebuilds_or_appends_at_owned_boundaries") {
        let steps = serde_json::from_value::<Input>(row.input).unwrap().steps;
        let expected: Vec<Step> = serde_json::from_value(row.expected).unwrap();
        assert_eq!(steps.len(), expected.len());
        let probe = Probe::new(true);
        let env = Env::with(
            Rc::new(palette_of(&theme_keys(), 0)),
            Rc::clone(&probe) as Rc<dyn SyntaxHighlighter>,
        );
        let mut last: Option<ComponentHandle> = None;
        for ((path, content, complete), step) in steps.into_iter().zip(expected) {
            let args = json!({"path": path, "content": content});
            let handle = draw_call(
                &args,
                &if complete {
                    settled(setup(true))
                } else {
                    setup(true)
                },
                &env,
                last.clone(),
            );
            assert_eq!(
                last.as_ref()
                    .is_some_and(|prior| Rc::ptr_eq(prior, &handle)),
                step.same
            );
            let calls: Vec<_> = probe.calls.borrow_mut().drain(..).collect();
            let operands: Vec<_> = step
                .highlight_inputs
                .iter()
                .map(|o| (o.code.clone(), o.lang.clone()))
                .collect();
            assert_eq!(calls, operands, "{args}");
            let lines: Vec<String> = step.lines.iter().map(|line| padded(line)).collect();
            assert_eq!(plain(&handle.render(WIDTH)), lines, "{args}");
            last = Some(handle);
        }
    }
}

#[test]
fn write_renderers_reuse_call_and_result_components() {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Expected {
        same_call: bool,
        call: Vec<String>,
        same_empty: bool,
        empty: Vec<String>,
        same_error: bool,
        error: Vec<String>,
    }
    let expected: Expected = serde_json::from_value(single_expected(
        "write_renderers_reuse_call_and_result_components",
    ))
    .unwrap();
    let _bindings = bind(&["ctrl+o".to_owned()]);
    let env = env();
    let padded_rows = |rows: &[String]| -> Vec<String> {
        rows.iter()
            .map(|row| padded(&unstyled_blank(row)))
            .collect()
    };
    let draw = |content: &str, last: Option<ComponentHandle>| {
        draw_call(
            &json!({"path": "x.txt", "content": content}),
            &setup(true),
            &env,
            last,
        )
    };
    let first = draw("before", None);
    let second = draw("after", Some(Rc::clone(&first)));
    assert_eq!(Rc::ptr_eq(&first, &second), expected.same_call);
    assert_eq!(first.render(WIDTH), padded_rows(&expected.call));

    let ok = result_of(vec![text_block("fine")]);
    let retained = Rc::new(Container::new());
    retained.add_child(Rc::new(Text::new("old".into(), 0, 0, None)));
    let kept: ComponentHandle = retained;
    assert_eq!(kept.render(WIDTH), laid_out("old"));
    let second = draw_result(&ok, &setup(false), &env, Some(Rc::clone(&kept)));
    assert_eq!(Rc::ptr_eq(&kept, &second), expected.same_empty);
    assert_eq!(second.render(WIDTH), padded_rows(&expected.empty));

    let mut failing = setup(false);
    failing.is_error = true;
    let first = draw_result(
        &result_of(vec![text_block("old error")]),
        &failing,
        &env,
        None,
    );
    let second = draw_result(
        &result_of(vec![text_block("new error")]),
        &failing,
        &env,
        Some(Rc::clone(&first)),
    );
    assert_eq!(Rc::ptr_eq(&first, &second), expected.same_error);
    assert_eq!(first.render(WIDTH), padded_rows(&expected.error));
}

#[test]
fn write_preview_callbacks_never_mutate_files() {
    let dir = Scratch::new().unwrap();
    let existing = dir.file("existing.txt", b"original");
    let ops = Arc::new(ScriptedWrite::new(&Journal::default()));
    let tool: Tool = create_write_tool_definition(
        dir.text(),
        WriteToolOptions {
            operations: Some(Arc::clone(&ops) as Arc<dyn WriteOperations>),
        },
    );
    let env = env();
    let _bindings = bind(&["ctrl+o".to_owned()]);
    let args = json!({"path": existing, "content": "replacement\nrows"});
    for complete in [false, true] {
        let ctx = context(
            &args,
            &if complete {
                settled(setup(true))
            } else {
                setup(true)
            },
            &env,
            None,
        );
        (tool.render_call.as_ref().unwrap())(Some(&args), &env.live, &ctx).unwrap();
    }
    let mut failing = settled(setup(false));
    failing.is_error = true;
    let ctx = context(&args, &failing, &env, None);
    let options = ToolRenderResultOptions {
        expanded: false,
        is_partial: false,
    };
    crate::display_support::native(
        tool.render_result.as_ref().unwrap(),
        &maestro_tools::output::render_utils::ToolRenderResultLike::from(&result_of(vec![
            text_block("denied"),
        ])),
        options,
        &env.live,
        &ctx,
    )
    .unwrap();
    assert_eq!(
        std::fs::read(dir.path().join("existing.txt")).unwrap(),
        b"original"
    );
    assert!(ops.calls().is_empty());
}

#[test]
fn write_rendering_uses_theme_roles_without_new_tokens() {
    let _bindings = bind(&["ctrl+o".to_owned()]);
    for name in ["dark", "light"] {
        let theme = display_support::shipped_theme(name, None);
        let env = Env::with(Rc::clone(&theme), native_highlighter());
        let fg = |color: ThemeColor, text: &str| theme.fg(&color, text).unwrap();
        let title = fg(
            ThemeColor::ToolTitle,
            &maestro_theme::Theme::bold("write", true),
        );
        let hint = key_hint("app.tools.expand", "to expand", &env.live).unwrap();

        let invalid = draw_call(&json!({"path": 1, "content": 2}), &setup(false), &env, None);
        let expected = format!(
            "{title} {}\n\n{}",
            fg(ThemeColor::Error, "[invalid arg]"),
            fg(ThemeColor::Error, "[invalid content arg - expected string]")
        );
        assert_eq!(invalid.render(WIDTH), laid_out(&expected), "{name} invalid");

        let body: Vec<String> = (0..11).map(|n| format!("r{n}")).collect();
        let args = json!({"path": "notes.txt", "content": body.join("\n")});
        let compact = draw_call(&args, &setup(false), &env, None);
        let shown: Vec<String> = body[..10]
            .iter()
            .map(|row| fg(ThemeColor::ToolOutput, row))
            .collect();
        let expected = format!(
            "{title} {}\n\n{}{} {hint})",
            fg(ThemeColor::Accent, "notes.txt"),
            shown.join("\n"),
            fg(ThemeColor::Muted, "\n... (1 more lines, 11 total,")
        );
        assert_eq!(compact.render(WIDTH), laid_out(&expected), "{name} compact");

        let mut failing = settled(setup(false));
        failing.is_error = true;
        let error = draw_result(&result_of(vec![text_block("boom")]), &failing, &env, None);
        let expected = format!("\n{}", fg(ThemeColor::Error, "boom"));
        assert_eq!(error.render(WIDTH), laid_out(&expected), "{name} error");

        let source = "fn main() {}";
        let code = draw_call(
            &json!({"path": "x.rs", "content": source}),
            &settled(setup(true)),
            &env,
            None,
        );
        let rendered = code.render(WIDTH);
        assert_eq!(strip_ansi(&rendered[2]).unwrap(), padded(source));
        let keyword = theme.get_fg_ansi(&ThemeColor::SyntaxKeyword).unwrap();
        assert!(rendered[2].contains(keyword), "{name} keyword color");
    }
}
