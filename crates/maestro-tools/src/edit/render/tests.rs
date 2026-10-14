//! Pure selection, formatting and layout witnesses from the rendering oracle.

use super::*;
#[path = "../../../../maestro-test-terminal/tests/support/manual_runtime.rs"]
#[allow(dead_code)] // shared host exposes operations unused by pure witnesses
mod manual_runtime;
#[path = "../../../tests/edit_render_support/mod.rs"]
mod support;
use maestro_theme::{
    LiveTheme, NativeThemeOperations, SyntectHighlighter, ThemeDirectories, ThemeState,
};
use serde_json::json;

/// Publish a controlled theme for one test.
fn live(theme: Rc<Theme>) -> LiveTheme {
    let state = ThemeState::new(
        ThemeDirectories {
            themes_dir: String::new(),
            custom_themes_dir: String::new(),
        },
        Rc::new(NativeThemeOperations),
    );
    state.set_theme_instance(theme).unwrap();
    state.theme()
}
/// A context with incomplete arguments so corpus layout performs no I/O.
fn context(args: &Value, theme: Rc<Theme>) -> ToolRenderContext<'_, EditRenderState> {
    ToolRenderContext {
        args,
        tool_call_id: "call".into(),
        invalidate: Rc::new(|| {}),
        runtime: Rc::new(manual_runtime::ManualRuntime::default()),
        last_component: None,
        state: Rc::new(RefCell::new(EditRenderState::default())),
        cwd: "/work".into(),
        phase: crate::types::ToolRenderPhase {
            execution_started: false,
            args_complete: false,
            is_partial: false,
        },
        expanded: false,
        show_images: true,
        is_error: false,
        readme_path: String::new(),
        live_theme: live(theme),
        highlighter: Rc::new(SyntectHighlighter::new().unwrap()),
        terminal_image: maestro_tui::TerminalImage::default(),
    }
}
/// Decode exactly the generated preview record or caught error record.
fn preview(value: &Value) -> Option<EditPreview> {
    if value.is_null() {
        return None;
    }
    Some(if value.get("error").is_some() {
        let error: support::Failure = serde_json::from_value(value.clone()).unwrap();
        Err(EditDiffError { error: error.error })
    } else {
        let details: support::Details = serde_json::from_value(value.clone()).unwrap();
        Ok(EditDiffResult {
            diff: details.diff,
            first_changed_line: details.first_changed_line,
        })
    })
}
/// Project a retained preview without internal request state.
fn preview_json(preview: Option<&EditPreview>) -> Value {
    match preview {
        None => Value::Null,
        Some(Ok(details)) => serde_json::to_value(details).unwrap(),
        Some(Err(error)) => json!({"error": error.error}),
    }
}
/// Convert a recorded result using the existing content boundary.
fn result(value: &Value) -> AgentToolResult<EditToolDetails> {
    let record: support::ResultRecord = serde_json::from_value(value.clone()).unwrap();
    AgentToolResult {
        terminate: None,
        content: record.content,
        details: record.details.map(|details| EditDiffResult {
            diff: details.diff,
            first_changed_line: details.first_changed_line,
        }),
    }
}

#[test]
fn edit_preview_selects_complete_inputs() {
    for case in support::cases("edit_preview_selects_complete_inputs") {
        assert_eq!(case.op, "select");
        let observed = preview_input(&case.input).map(|input| json!({"path": input.path, "edits": input.edits.iter().map(|edit| json!({"oldText": edit.old_text, "newText": edit.new_text})).collect::<Vec<_>>() }));
        assert_eq!(observed.unwrap_or(Value::Null), case.expected);
    }
}
#[test]
fn edit_title_uses_display_alias_precedence() {
    let theme = support::palette(0);
    for case in support::cases("edit_title_uses_display_alias_precedence") {
        assert_eq!(case.op, "title");
        assert_eq!(
            json!(format_call(&case.input, &theme).unwrap()),
            case.expected
        );
    }
}
#[test]
fn edit_header_selects_preview_before_settled_error() {
    let theme = support::palette(0);
    for case in support::cases("edit_header_selects_preview_before_settled_error") {
        assert_eq!(case.op, "background");
        let state = PreviewState {
            preview: preview(&case.input["preview"]),
            settled_error: case.input["settled"].as_bool().unwrap(),
            ..PreviewState::default()
        };
        assert_eq!(
            json!(theme.bg(&header_color(&state), "ROW").unwrap()),
            case.expected
        );
    }
}
/// Consume every result query through its owned formatting operation.
fn result_cases(name: &str) {
    let theme = Rc::new(support::palette(0));
    let args = json!({});
    for case in support::cases(name) {
        assert_eq!(case.op, "result");
        let mut ctx = context(&args, Rc::clone(&theme));
        ctx.is_error = case.input["isError"].as_bool().unwrap();
        assert_eq!(
            json!(
                format_result(
                    preview(&case.input["preview"]).as_ref(),
                    &result(&case.input["result"]),
                    &theme,
                    &ctx
                )
                .unwrap()
            ),
            case.expected
        );
    }
}
#[test]
fn edit_result_errors_keep_text_order_and_suppress_duplicates() {
    result_cases("edit_result_errors_keep_text_order_and_suppress_duplicates");
}
#[test]
fn edit_result_success_only_emits_a_distinct_nonempty_diff() {
    result_cases("edit_result_success_only_emits_a_distinct_nonempty_diff");
}
#[test]
fn edit_preview_changes_compare_variant_text_and_first_line() {
    for case in support::cases("edit_preview_changes_compare_variant_text_and_first_line") {
        assert_eq!(case.op, "set");
        let mut state = PreviewState {
            preview: preview(&case.input["old"]),
            request: Some(Rc::new(())),
            ..PreviewState::default()
        };
        let changed = set_preview(&mut state, preview(&case.input["next"]).unwrap());
        assert_eq!(
            json!({"changed": changed, "preview": preview_json(state.preview.as_ref()), "pending": state.request.is_some()}),
            case.expected
        );
    }
}
/// Compare full composed frames, leaving wrapping to the existing widgets.
fn box_cases(name: &str) {
    for case in support::cases(name)
        .into_iter()
        .filter(|case| matches!(case.op.as_str(), "box" | "brand"))
    {
        let theme = Rc::new(if case.op == "brand" {
            support::theme(&case.input["colors"], &case.input["backgrounds"])
        } else {
            support::palette(0)
        });
        let ctx = context(&case.input["args"], Rc::clone(&theme));
        let call = call_component(&ctx);
        call.state.borrow_mut().preview = preview(&case.input["preview"]);
        build_call(&call, ctx.args, &theme, &ctx).unwrap();
        assert_eq!(
            json!(call.render(usize::try_from(case.input["width"].as_u64().unwrap()).unwrap())),
            case.expected
        );
    }
}
#[test]
fn edit_box_layout_keeps_padding_and_preview_spacing() {
    box_cases("edit_box_layout_keeps_padding_and_preview_spacing");
    box_cases("edit_theme_replacement_updates_background_and_rebuilt_text");
}
#[test]
fn edit_box_layout_wraps_through_existing_widgets() {
    box_cases("edit_box_layout_wraps_through_existing_widgets");
}
