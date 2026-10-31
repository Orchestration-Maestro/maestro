//! Retained edit calls, result replay and preview ownership through public callbacks.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use crate::display_support;
use crate::edit_render_support;
use crate::edit_support;
use crate::read_support;
use crate::runtime;

use display_support::{Env, NoLanguages, Setup, context};
use edit_render_support::{Details, ResultRecord, cases, palette, theme};
use maestro_agent::AgentToolResult;
use maestro_tools::{
    EditRenderState, EditToolDetails, EditToolInput, EditToolOptions, ToolDefinition,
    ToolRenderContext, ToolRenderResultOptions, create_edit_tool_definition,
};
use maestro_tui::{Text, tui::ComponentHandle};
use read_support::{Fixture, Scratch};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

/// Definition under test.
type Tool = ToolDefinition<(), EditToolInput, EditToolDetails, EditRenderState>;
/// One row with a controlled runtime and local file.
struct Row {
    /// Definition cwd decoy.
    tool: Tool,
    /// Controlled live theme and runtime.
    env: Env,
    /// Directory used by previews.
    dir: Scratch,
    /// Shared renderer state.
    state: Rc<RefCell<EditRenderState>>,
    /// Accepted completion notifications.
    invalidations: Rc<Cell<usize>>,
}
impl Row {
    /// A controlled row with distinguishable source text.
    fn new() -> Self {
        let dir = Scratch::new().unwrap();
        dir.file("file", b"alpha\n");
        Self {
            tool: create_edit_tool_definition(
                "/definition-decoy".into(),
                EditToolOptions::default(),
            ),
            env: Env::with(Rc::new(palette(0)), Rc::new(NoLanguages)),
            dir,
            state: Rc::new(RefCell::new(EditRenderState::default())),
            invalidations: Rc::new(Cell::new(0)),
        }
    }
    /// Construct a context without retaining the runtime in its invalidator.
    fn context<'a>(
        &self,
        args: &'a Value,
        last: Option<ComponentHandle>,
    ) -> ToolRenderContext<'a, EditRenderState> {
        let setup = Setup::from(
            &json!({"readme":"/docs/README.md", "bindings":[], "cwd":self.dir.text(), "argsComplete":true}),
        );
        let mut ctx = context(args, &setup, &self.env, last);
        ctx.state = Rc::clone(&self.state);
        let count = Rc::clone(&self.invalidations);
        ctx.invalidate = Rc::new(move || count.set(count.get() + 1));
        ctx
    }
    /// Render with this row's retained state.
    fn call(&self, args: &Value, last: Option<ComponentHandle>) -> ComponentHandle {
        (self.tool.render_call.as_ref().unwrap())(
            Some(args),
            &self.env.live,
            &self.context(args, last),
        )
        .unwrap()
    }
    /// Settle the row, preserving its separate result component.
    fn result(
        &self,
        args: &Value,
        result: &AgentToolResult<EditToolDetails>,
        error: bool,
        last: Option<ComponentHandle>,
    ) -> ComponentHandle {
        let mut ctx = self.context(args, last);
        ctx.is_error = error;
        crate::display_support::native(
            self.tool.render_result.as_ref().unwrap(),
            &maestro_tools::output::render_utils::ToolRenderResultLike::from(result),
            ToolRenderResultOptions {
                expanded: false,
                is_partial: false,
            },
            &self.env.live,
            &ctx,
        )
        .unwrap()
    }
    /// Finish every submitted producer and close the notification observation set.
    async fn finish(&self) {
        for future in self.env.runtime.take_futures() {
            future.await.unwrap();
        }
    }
}
/// Complete authored arguments.
fn args(new: &str) -> Value {
    json!({"path":"file", "edits":[{"oldText":"alpha", "newText":new}]})
}
/// A successful result with optional details.
fn result(diff: Option<&str>) -> AgentToolResult<EditToolDetails> {
    AgentToolResult::text_result(
        "execution message".into(),
        diff.map(|diff| EditToolDetails {
            diff: diff.into(),
            first_changed_line: Some(1),
        }),
    )
}
/// Convert strictly recorded details through the existing native result type.
fn recorded_result(value: &Value) -> AgentToolResult<EditToolDetails> {
    let record: ResultRecord = serde_json::from_value(value.clone()).unwrap();
    AgentToolResult {
        content: record.content,
        details: record.details.map(|details| EditToolDetails {
            diff: details.diff,
            first_changed_line: details.first_changed_line,
        }),
        terminate: None,
    }
}
/// Visible output, preserving order and repetition.
fn plain(call: &ComponentHandle) -> String {
    call.render(80)
        .iter()
        .map(|row| maestro_tools::output::render_utils::strip_ansi(row).unwrap())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn edit_renderers_retain_call_and_result_slots() {
    let first = Row::new();
    let second = Row::new();
    let incomplete = json!({"path":"one"});
    let a = first.call(&incomplete, None);
    let b = second.call(&incomplete, None);
    assert!(!Rc::ptr_eq(&a, &b));
    assert!(Rc::ptr_eq(&a, &first.call(&incomplete, None)));
    assert!(Rc::ptr_eq(
        &a,
        &second.call(&incomplete, Some(Rc::clone(&a)))
    ));
    assert!(Rc::ptr_eq(&a, &second.call(&incomplete, None)));
    let unrelated: ComponentHandle = Rc::new(Text::new("unrelated".into(), 0, 0, None));
    assert!(Rc::ptr_eq(&a, &second.call(&incomplete, Some(unrelated))));
    let detached = Row::new();
    let out = detached.result(&incomplete, &result(Some("+1 changed")), false, None);
    assert!(!out.render(40).is_empty());
    let empty = detached.result(&incomplete, &result(None), false, Some(Rc::clone(&out)));
    assert!(Rc::ptr_eq(&out, &empty));
    assert!(empty.render(40).is_empty());
}

#[test]
fn edit_preview_waits_for_complete_args_and_deduplicates() {
    runtime::run(async {
        let row = Row::new();
        for invalid in [
            Value::Null,
            json!({}),
            json!({"path":"file", "edits":[null]}),
        ] {
            row.call(&invalid, None);
            assert_eq!(row.env.runtime.futures(), 0);
        }
        let input = args("beta");
        let mut ctx = row.context(&input, None);
        ctx.phase.args_complete = false;
        (row.tool.render_call.as_ref().unwrap())(Some(&input), &row.env.live, &ctx).unwrap();
        assert_eq!(row.env.runtime.futures(), 0);
        let call = row.call(&input, None);
        for flag in 0..3 {
            let mut ctx = row.context(&input, Some(Rc::clone(&call)));
            match flag {
                0 => ctx.phase.execution_started = true,
                1 => ctx.expanded = true,
                _ => ctx.phase.is_partial = true,
            }
            (row.tool.render_call.as_ref().unwrap())(Some(&input), &row.env.live, &ctx).unwrap();
            assert_eq!(row.env.runtime.futures(), 1);
        }
        let mut alias = input.clone();
        alias["file_path"] = json!("display-only");
        row.call(&alias, None);
        assert_eq!(row.env.runtime.futures(), 1);
        row.finish().await;
        row.call(&alias, None);
        assert_eq!(row.env.runtime.futures(), 0);
        assert_eq!(row.invalidations.get(), 1);
        assert!(plain(&call).contains("beta"));
        let failure = args("other");
        row.dir.file("file", b"no match\n");
        row.call(&failure, None);
        row.finish().await;
        row.call(&failure, None);
        assert_eq!(row.env.runtime.futures(), 0);
    });
}

#[test]
fn edit_changed_inputs_replace_pending_request() {
    runtime::run(async {
        let row = Row::new();
        row.dir.file("other", b"alpha\n");
        let original = args("beta");
        let call = row.call(&original, None);
        let mut inputs = vec![args("gamma")];
        let mut path = inputs.last().unwrap().clone();
        path["path"] = json!("other");
        inputs.push(path);
        let mut old_text = inputs.last().unwrap().clone();
        old_text["edits"][0]["oldText"] = json!("alpha\n");
        inputs.push(old_text);
        inputs.push(json!({"path":"file", "edits":[{"oldText":"p","newText":"P"},{"oldText":"l","newText":"L"}]}));
        inputs.push(json!({"path":"file", "edits":[{"oldText":"l","newText":"L"},{"oldText":"p","newText":"P"}]}));
        for (index, input) in inputs.iter().enumerate() {
            row.call(input, None);
            assert_eq!(row.env.runtime.futures(), index + 2);
        }
        let mut futures = row.env.runtime.take_futures();
        assert_eq!(futures.len(), inputs.len() + 1);
        let latest = futures.pop().unwrap();
        latest.await.unwrap();
        for old in futures {
            old.await.unwrap();
        }
        row.call(inputs.last().unwrap(), None);
        assert_eq!(row.invalidations.get(), 1);
        assert!(plain(&call).contains("aLPha"));
        row.result(
            inputs.last().unwrap(),
            &result(Some("+1 settled")),
            false,
            None,
        );
        row.call(&original, None);
        assert_eq!(row.env.runtime.futures(), 1);
        row.finish().await;
    });
}

#[test]
fn edit_incomplete_arguments_discard_pending_preview() {
    runtime::run(async {
        let row = Row::new();
        let input = args("beta");
        let call = row.call(&input, None);
        row.call(&json!({"path":"file"}), None);
        row.finish().await;
        assert_eq!(row.invalidations.get(), 0);
        assert!(!plain(&call).contains("beta"));
        row.call(&input, None);
        row.finish().await;
        row.call(&input, None);
        assert!(plain(&call).contains("beta"));
        row.call(&json!({}), None);
        assert!(!plain(&call).contains("beta"));
    });
}

#[test]
fn edit_completion_publishes_before_reentrant_invalidation() {
    runtime::run(async {
        let row = Rc::new(Row::new());
        let input = args("beta");
        let weak = Rc::downgrade(&row);
        let observed = Rc::new(Cell::new(false));
        let capture = Rc::clone(&observed);
        let mut ctx = row.context(&input, None);
        ctx.invalidate = Rc::new(move || {
            let row = weak.upgrade().unwrap();
            let call = row.call(&args("beta"), None);
            capture.set(plain(&call).contains("beta"));
            assert_eq!(row.env.runtime.futures(), 0);
        });
        let _call =
            (row.tool.render_call.as_ref().unwrap())(Some(&input), &row.env.live, &ctx).unwrap();
        row.finish().await;
        assert!(observed.get());
    });
}

#[test]
fn edit_settled_result_rejects_late_preview() {
    runtime::run(async {
        for (error, diff, partial) in [
            (false, Some("+1 SETTLED"), false),
            (true, None, false),
            (false, Some(""), false),
            (false, None, false),
            (false, Some("+1 PARTIAL"), true),
        ] {
            for bytes in [b"alpha\n".as_slice(), b"unmatched\n".as_slice()] {
                let row = Row::new();
                let input = args("beta");
                let call = row.call(&input, None);
                let mut ctx = row.context(&input, None);
                ctx.is_error = error;
                ctx.phase.is_partial = partial;
                crate::display_support::native(
                    row.tool.render_result.as_ref().unwrap(),
                    &maestro_tools::output::render_utils::ToolRenderResultLike::from(&result(diff)),
                    ToolRenderResultOptions {
                        expanded: false,
                        is_partial: partial,
                    },
                    &row.env.live,
                    &ctx,
                )
                .unwrap();
                let settled = call.render(80);
                row.dir.file("file", bytes);
                row.finish().await;
                row.call(&input, None);
                assert_eq!(call.render(80), settled);
                assert_eq!(row.invalidations.get(), 0);
                assert_eq!(row.env.runtime.futures(), 0);
            }
        }
    });
}

#[test]
fn edit_returned_arguments_reject_obsolete_generation() {
    runtime::run(async {
        let row = Row::new();
        let a = args("newest A");
        let call = row.call(&a, None);
        row.call(&args("B"), None);
        row.call(&a, None);
        let mut futures = row.env.runtime.take_futures();
        assert_eq!(futures.len(), 3);
        futures.pop().unwrap().await.unwrap();
        row.dir.file("file", b"unmatched\n");
        for future in futures {
            future.await.unwrap();
        }
        row.call(&a, None);
        assert!(plain(&call).contains("newest A"));
        assert!(!plain(&call).contains("Could not find"));
        assert_eq!(row.invalidations.get(), 1);
    });
}

/// One native-file observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeCase {
    /// Owning callback witness.
    name: String,
    /// Authored arguments.
    input: Value,
    /// Original file bytes.
    file: Option<String>,
    /// Independent result/frame observations.
    expected: NativeExpected,
}
/// Recorded native outputs; optional members distinguish preview and replay.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NativeExpected {
    /// Preview diff or caught error.
    preview: Value,
    /// Full call frame.
    rows: Vec<String>,
    /// Accepted notifications.
    invalidations: Option<usize>,
    /// Preserved source bytes.
    file_after: Option<String>,
    /// Result frame after settlement.
    result_rows: Option<Vec<String>>,
    /// Whether replay read or recreated the deleted source.
    file_exists: Option<bool>,
}
/// Consume a native fixture through the public callbacks and real captured futures.
async fn native(name: &str) {
    let cases: Vec<NativeCase> =
        serde_json::from_str(include_str!("fixtures/edit-render-native.json")).unwrap();
    assert_eq!(cases.len(), 4);
    let case = cases.into_iter().find(|case| case.name == name).unwrap();
    let mut row = Row::new();
    let definition_dir = Scratch::new().unwrap();
    definition_dir.file("file", b"definition decoy\n");
    let operations = Arc::new(edit_support::Operations::new(b"execution decoy\n"));
    row.tool = edit_support::tool(&operations, &definition_dir.text());
    if let Some(file) = &case.file {
        row.dir.file("file", file.as_bytes());
    } else {
        row.dir.file(
            "file",
            (0..200)
                .fold(String::new(), |mut text, index| {
                    use std::fmt::Write;
                    writeln!(text, "line {index}").unwrap();
                    text
                })
                .as_bytes(),
        );
    }
    let mut ctx = row.context(&case.input, None);
    ctx.phase.args_complete = case.expected.file_exists.is_none();
    let call =
        (row.tool.render_call.as_ref().unwrap())(Some(&case.input), &row.env.live, &ctx).unwrap();
    if let Some(exists) = case.expected.file_exists {
        replay_native(&row, &case, exists);
    } else {
        preview_native(&row, &case, &call).await;
    }
    assert_eq!(call.render(80), case.expected.rows);
    assert!(operations.events.entries().is_empty());
    assert!(operations.script.calls().is_empty());
    assert_eq!(
        std::fs::read_to_string(definition_dir.path().join("file")).unwrap(),
        "definition decoy\n"
    );
    if let Some(expected) = case.expected.invalidations {
        assert_eq!(row.invalidations.get(), expected);
    }
}
/// Reconstruct a native replay after deleting its source.
fn replay_native(row: &Row, case: &NativeCase, exists: bool) {
    std::fs::remove_file(row.dir.path().join("file")).unwrap();
    let details: Details = serde_json::from_value(case.expected.preview.clone()).unwrap();
    let out = row.result(
        &case.input,
        &AgentToolResult {
            content: vec![],
            details: Some(EditToolDetails {
                diff: details.diff,
                first_changed_line: details.first_changed_line,
            }),
            terminate: None,
        },
        false,
        None,
    );
    assert_eq!(out.render(80), *case.expected.result_rows.as_ref().unwrap());
    assert_eq!(row.dir.path().join("file").exists(), exists);
    assert_eq!(row.env.runtime.futures(), 0);
}
/// Complete and compare the native preview and optional result.
async fn preview_native(row: &Row, case: &NativeCase, call: &ComponentHandle) {
    row.finish().await;
    assert!(Rc::ptr_eq(call, &row.call(&case.input, None)));
    assert_eq!(call.render(80), case.expected.rows);
    assert_eq!(
        std::fs::read_to_string(row.dir.path().join("file")).unwrap(),
        case.expected.file_after.as_ref().unwrap().as_str()
    );
    if let Some(expected) = &case.expected.result_rows {
        let details: Details = serde_json::from_value(case.expected.preview.clone()).unwrap();
        let out = row.result(
            &case.input,
            &AgentToolResult {
                content: result(None).content,
                details: Some(EditToolDetails {
                    diff: details.diff,
                    first_changed_line: details.first_changed_line,
                }),
                terminate: None,
            },
            false,
            None,
        );
        assert_eq!(&out.render(80), expected);
        assert!(Rc::ptr_eq(call, &row.call(&case.input, None)));
        assert_eq!(call.render(80), case.expected.rows);
    } else if let Some(error) = case.expected.preview.get("error") {
        assert!(plain(call).contains(error.as_str().unwrap().split(" including").next().unwrap()));
    } else {
        let details: Details = serde_json::from_value(case.expected.preview.clone()).unwrap();
        let expected = maestro_tools::render_diff(
            &details.diff,
            &maestro_tools::RenderDiffOptions::default(),
            &row.env.live,
        )
        .unwrap();
        assert!(
            call.render(1000)
                .join("\n")
                .contains(expected.lines().next().unwrap())
        );
    }
}

#[test]
fn edit_preview_reads_local_file_without_execution() {
    runtime::run(async {
        native("edit_preview_reads_local_file_without_execution").await;
    });
}
#[test]
fn edit_preflight_error_replaces_preview_body() {
    runtime::run(async {
        native("edit_preflight_error_replaces_preview_body").await;
    });
}
#[test]
fn edit_large_preview_settles_without_duplicate_content() {
    runtime::run(async {
        native("edit_large_preview_settles_without_duplicate_content").await;
    });
}
#[test]
fn edit_replay_reconstructs_preview_without_reading() {
    runtime::run(async {
        native("edit_replay_reconstructs_preview_without_reading").await;
    });
}

#[test]
fn edit_result_without_call_renders_detached_output() {
    let row = Row::new();
    let input = json!({});
    let mut previous = None;
    for case in cases("edit_result_without_call_renders_detached_output") {
        assert_eq!(case.op, "detached");
        let mut ctx = row.context(&input, previous.clone());
        ctx.expanded = case.input["context"]["expanded"].as_bool().unwrap();
        ctx.phase.is_partial = case.input["context"]["isPartial"].as_bool().unwrap();
        let options = ToolRenderResultOptions {
            expanded: case.input["options"]["expanded"].as_bool().unwrap(),
            is_partial: case.input["options"]["isPartial"].as_bool().unwrap(),
        };
        let out = crate::display_support::native(
            row.tool.render_result.as_ref().unwrap(),
            &maestro_tools::output::render_utils::ToolRenderResultLike::from(&recorded_result(
                &case.input["result"],
            )),
            options,
            &row.env.live,
            &ctx,
        )
        .unwrap();
        assert_eq!(json!(out.render(40)), case.expected);
        if let Some(previous) = previous {
            assert!(Rc::ptr_eq(&previous, &out));
        }
        previous = Some(out);
    }
    for error in [false, true] {
        let empty = AgentToolResult {
            content: vec![],
            details: None,
            terminate: None,
        };
        let out = row.result(&input, &empty, error, previous.clone());
        assert!(Rc::ptr_eq(previous.as_ref().unwrap(), &out));
        assert!(out.render(40).is_empty());
    }
    assert_eq!(row.env.runtime.futures(), 0);
}

#[test]
fn edit_settlement_preserves_existing_preview_priority() {
    runtime::run(async {
        let row = Row::new();
        let input = args("beta");
        let call = row.call(&input, None);
        row.finish().await;
        row.call(&input, None);
        let before = call.render(80);
        row.result(&input, &result(None), true, None);
        assert_eq!(call.render(80), before);
        let changed_args = json!({"file_path":"current-title"});
        row.result(&changed_args, &result(Some("+1 replacement")), false, None);
        assert!(plain(&call).contains("current-title"));
        assert!(plain(&call).contains("replacement"));
        let mut line = result(Some("+1 replacement"));
        line.details.as_mut().unwrap().first_changed_line = Some(9);
        let title = json!({"path":"changed-line-title"});
        row.result(&title, &line, false, None);
        assert!(plain(&call).contains("changed-line-title"));
        row.result(
            &json!({"path":"equal-must-not-rebuild"}),
            &line,
            false,
            None,
        );
        assert!(plain(&call).contains("changed-line-title"));
        row.result(&title, &result(Some("")), false, None);
        assert!(!plain(&call).contains("replacement"));
        let failed = Row::new();
        failed.dir.file("file", b"unmatched");
        let call = failed.call(&input, None);
        failed.finish().await;
        failed.call(&input, None);
        let before = call.render(80);
        failed.result(&input, &result(None), false, None);
        assert_eq!(call.render(80), before);
        failed.result(&input, &result(Some("+1 recovered")), true, None);
        assert_eq!(call.render(80), before);
    });
}

/// Keep the supplied title palette distinct from the live diff palette.
fn supplied_theme_routes() {
    for case in cases("edit_theme_replacement_updates_background_and_rebuilt_text")
        .into_iter()
        .filter(|case| case.op == "theme_routes")
    {
        let row = Row::new();
        assert_eq!(case.op, "theme_routes");
        let supplied = maestro_theme::LiveTheme::from_theme(Rc::new(theme(
            &case.input["titleColors"],
            &json!({}),
        )));
        let mut ctx = row.context(&case.input["args"], None);
        ctx.phase.args_complete = false;
        let call = (row.tool.render_call.as_ref().unwrap())(ctx.args, &supplied, &ctx).unwrap();
        assert!(
            call.render(1000)
                .join("\n")
                .contains(case.expected["title"].as_str().unwrap())
        );
        let out = crate::display_support::native(
            row.tool.render_result.as_ref().unwrap(),
            &maestro_tools::output::render_utils::ToolRenderResultLike::from(&recorded_result(
                &case.input["result"],
            )),
            ToolRenderResultOptions {
                expanded: false,
                is_partial: false,
            },
            &supplied,
            &ctx,
        )
        .unwrap();
        assert!(out.render(1000).is_empty());
        for line in case.expected["diff"].as_str().unwrap().lines() {
            assert!(call.render(1000).join("\n").contains(line));
        }
    }
}

#[test]
fn edit_theme_replacement_updates_background_and_rebuilt_text() {
    runtime::run(async {
        supplied_theme_routes();
        let row = Row::new();
        let input = json!({"path":"file"});
        let call = row.call(&input, None);
        let before = call.render(80);
        row.env
            .state
            .set_theme_instance(Rc::new(palette(20)))
            .unwrap();
        let live = call.render(80);
        assert_ne!(before, live);
        assert!(live[1].contains("38;5;1m"));
        let ctx = row.context(&input, None);
        let supplied = maestro_theme::LiveTheme::from_theme(Rc::new(palette(20)));
        (row.tool.render_call.as_ref().unwrap())(Some(&input), &supplied, &ctx).unwrap();
        assert!(call.render(80)[1].contains("38;5;21m"));
        let failed = Row::new();
        failed.dir.file("file", b"unmatched");
        let input = args("beta");
        let call = failed.call(&input, None);
        failed.finish().await;
        failed.call(&input, None);
        assert!(call.render(80)[0].contains("48;5;13m"));
        failed
            .env
            .state
            .set_theme_instance(Rc::new(palette(20)))
            .unwrap();
        assert!(call.render(80)[0].contains("48;5;33m"));
        let selected = maestro_theme::LiveTheme::from_theme(Rc::new(palette(20)));
        let ctx = failed.context(&input, None);
        (failed.tool.render_call.as_ref().unwrap())(Some(&input), &selected, &ctx).unwrap();
        assert!(call.render(80).join("\n").contains("38;5;24m"));
    });
}

#[test]
fn edit_preview_teardown_releases_owned_callbacks() {
    runtime::run(async {
        let mut row = Row::new();
        let input = args("beta");
        let alias = row.call(&input, None);
        row.state = Rc::new(RefCell::new(EditRenderState::default()));
        row.finish().await;
        row.call(&input, Some(Rc::clone(&alias)));
        assert!(plain(&alias).contains("beta"));
        assert_eq!(row.invalidations.get(), 1);
        let mut dead = Row::new();
        let call = dead.call(&input, None);
        let weak = Rc::downgrade(&call);
        dead.state = Rc::new(RefCell::new(EditRenderState::default()));
        drop(call);
        assert!(weak.upgrade().is_none());
        dead.finish().await;
        assert_eq!(dead.invalidations.get(), 0);
        let queued = Row::new();
        let capture = Rc::new(());
        let weak_capture = Rc::downgrade(&capture);
        let mut ctx = queued.context(&input, None);
        ctx.invalidate = Rc::new(move || {
            let _ = &capture;
        });
        let call =
            (queued.tool.render_call.as_ref().unwrap())(Some(&input), &queued.env.live, &ctx)
                .unwrap();
        drop(ctx);
        drop(call);
        drop(queued);
        assert!(weak_capture.upgrade().is_none());
    });
}

/// Check the exact selected-role failure after a real preview completes.
async fn selected_preview_theme_error(missing: &str) {
    let row = Row::new();
    if missing == "toolErrorBg" {
        row.dir.file("file", b"unmatched");
    }
    let input = args("beta");
    let call = row.call(&input, None);
    row.finish().await;
    let selected = Rc::new(edit_render_support::without(missing));
    row.env
        .state
        .set_theme_instance(Rc::clone(&selected))
        .unwrap();
    let ctx = row.context(&input, None);
    let error = (row.tool.render_call.as_ref().unwrap())(Some(&input), &row.env.live, &ctx)
        .err()
        .unwrap();
    let kind = if missing.ends_with("Bg") {
        "background color"
    } else {
        "color"
    };
    assert_eq!(error.message, format!("Unknown theme {kind}: {missing}"));
    assert_eq!(row.env.runtime.futures(), 0);
    drop(call);
}

#[test]
fn edit_renderer_propagates_selected_theme_errors() {
    for missing in ["error", "toolTitle", "accent", "toolPendingBg"] {
        let row = Row::new();
        let input = args("beta");
        let selected = Rc::new(edit_render_support::without(missing));
        row.env
            .state
            .set_theme_instance(Rc::clone(&selected))
            .unwrap();
        let ctx = row.context(&input, None);
        let error = (row.tool.render_call.as_ref().unwrap())(Some(&input), &row.env.live, &ctx)
            .err()
            .unwrap();
        let kind = if missing.starts_with("toolPending") {
            "background color"
        } else {
            "color"
        };
        assert_eq!(error.message, format!("Unknown theme {kind}: {missing}"));
        assert_eq!(row.env.runtime.futures(), 1);
        drop(row.env.runtime.take_futures());
    }
    runtime::run(async {
        for missing in ["toolDiffAdded", "toolSuccessBg", "toolErrorBg"] {
            selected_preview_theme_error(missing).await;
        }
    });
    let row = Row::new();
    let input = json!({"path":"file"});
    let selected = Rc::new(edit_render_support::without("toolDiffAdded"));
    row.env
        .state
        .set_theme_instance(Rc::clone(&selected))
        .unwrap();
    let ctx = row.context(&input, None);
    let call = (row.tool.render_call.as_ref().unwrap())(Some(&input), &row.env.live, &ctx).unwrap();
    assert!(call.render(80)[1].contains("38;5;1m"));
    row.env
        .state
        .set_theme_instance(Rc::new(display_support::bare_theme()))
        .unwrap();
    assert!(!call.render(80)[0].contains("48;"));
}
