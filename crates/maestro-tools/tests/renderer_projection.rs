//! Typed and persisted rendering through the same callback slot.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::display_support::{Env, Setup, context};
use maestro_tools::{
    ToolRenderResult, ToolRenderResultOptions, output::render_utils::ToolRenderResultLike,
};
use maestro_tui::Text;
use serde_json::json;
use std::{cell::Cell, rc::Rc};

#[test]
fn native_renderer_keeps_typed_details_without_serializing() {
    let env = Env::new(0);
    let args = json!({});
    let ctx = context::<()>(
        &args,
        &Setup::from(&json!({"readme":"/opt/maestro/README.md","bindings":[]})),
        &env,
        None,
    );
    let called = Rc::new(Cell::new(false));
    let observed = Rc::clone(&called);
    let callback = ToolRenderResult::Typed(Rc::new(
        move |result: &ToolRenderResultLike<'_, u32>, options, _, context| {
            assert_eq!(result.details, Some(&42));
            assert_eq!(result.terminate, Some(true));
            assert_eq!(result.is_error, Some(false));
            assert!(result.content.is_empty());
            assert!(options.expanded);
            assert!(!options.is_partial);
            assert!(!context.expanded);
            observed.set(true);
            Ok(Rc::new(Text::new("typed".into(), 0, 0, None)))
        },
    ));
    callback
        .invoke_native(
            (
                &ToolRenderResultLike {
                    content: &[],
                    details: Some(&42),
                    terminate: Some(true),
                    is_error: Some(false),
                },
                ToolRenderResultOptions {
                    expanded: true,
                    is_partial: false,
                },
            ),
            &env.theme,
            &ctx,
            |_| panic!("typed callback must not serialize"),
        )
        .unwrap();
    assert!(called.get());
}

#[test]
fn renderer_projection_details_roots_keep_absence_and_unknown_fields() {
    crate::renderer_projection_support::check(0..42);
}

#[test]
fn renderer_projection_truncation_defaults_keep_sparse_records() {
    crate::renderer_projection_support::check(42..87);
}
#[test]
fn renderer_projection_limit_counts_keep_units_and_warning_order() {
    crate::renderer_projection_support::check(87..105);
}
#[test]
fn renderer_projection_artifacts_precede_cut_notice() {
    crate::renderer_projection_support::check(105..109);
}
#[test]
fn renderer_projection_line_flags_only_add_selected_notice() {
    crate::renderer_projection_support::check(109..112);
}
#[test]
fn renderer_projection_edit_diff_preserves_error_and_empty_rules() {
    crate::renderer_projection_support::check(112..119);
}
#[test]
fn renderer_projection_expanded_results_keep_existing_layout() {
    crate::renderer_projection_support::check(119..126);
}

/// A borrowed result preserving present null and the host flags.
fn raw(details: Option<&serde_json::Value>) -> ToolRenderResultLike<'_, serde_json::Value> {
    ToolRenderResultLike {
        content: &[],
        details,
        terminate: Some(false),
        is_error: Some(true),
    }
}
/// A standalone context and controlled display environment.
fn environment() -> (Env, Setup) {
    (
        Env::new(0),
        Setup::from(&json!({"readme":"/opt/maestro/README.md","bindings":[]})),
    )
}
/// A callback observation independent of details representation.
fn empty() -> maestro_tui::tui::ComponentHandle {
    Rc::new(Text::new("observed".into(), 0, 0, None))
}
/// Distinguishable conversion error.
fn failure() -> maestro_models::DiagnosticErrorInfo {
    maestro_models::DiagnosticErrorInfo::error("conversion failed")
}
#[test]
fn native_raw_renderer_serializes_only_present_details() {
    let (env, setup) = environment();
    let args = json!({});
    let ctx = context::<()>(&args, &setup, &env, None);
    let observed = Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = Rc::clone(&observed);
    let callback = ToolRenderResult::<u32, ()>::Raw(Rc::new(move |result, _, _, _| {
        seen.borrow_mut().push(result.details.cloned());
        assert_eq!(result.terminate, Some(false));
        assert_eq!(result.is_error, Some(true));
        Ok(empty())
    }));
    let count = Cell::new(0);
    let options = ToolRenderResultOptions {
        expanded: false,
        is_partial: true,
    };
    for details in [None, Some(&7)] {
        callback
            .invoke_native(
                (
                    &ToolRenderResultLike {
                        content: &[],
                        details,
                        terminate: Some(false),
                        is_error: Some(true),
                    },
                    options,
                ),
                &env.theme,
                &ctx,
                |_| {
                    count.set(count.get() + 1);
                    Ok(serde_json::Value::Null)
                },
            )
            .unwrap();
    }
    assert_eq!(count.get(), 1);
    assert_eq!(*observed.borrow(), [None, Some(serde_json::Value::Null)]);
    let result = ToolRenderResultLike {
        content: &[],
        details: Some(&7),
        terminate: None,
        is_error: None,
    };
    let error = callback
        .invoke_native((&result, options), &env.theme, &ctx, |_| Err(failure()))
        .err()
        .unwrap();
    assert_eq!(error.message, "conversion failed");
    assert_eq!(observed.borrow().len(), 2);
}
#[test]
fn raw_renderer_does_not_invoke_typed_decoder() {
    let (env, setup) = environment();
    let args = json!({});
    let ctx = context::<()>(&args, &setup, &env, None);
    let value = json!({"unknown":{"nested":[1,false]}});
    let observed = Cell::new(0);
    let pointer = &raw const value;
    let callback =
        ToolRenderResult::<std::convert::Infallible, ()>::Raw(Rc::new(move |result, _, _, _| {
            assert!(std::ptr::eq(result.details.unwrap(), pointer));
            assert_eq!(
                result.details.unwrap(),
                &json!({"unknown":{"nested":[1,false]}})
            );
            Ok(empty())
        }));
    callback
        .invoke_raw(
            (
                &raw(Some(&value)),
                ToolRenderResultOptions {
                    expanded: false,
                    is_partial: false,
                },
            ),
            &env.theme,
            &ctx,
            |_| {
                observed.set(observed.get() + 1);
                Err(failure())
            },
        )
        .unwrap();
    assert_eq!(observed.get(), 0);
}
#[test]
fn typed_renderer_decodes_only_present_details() {
    let (env, setup) = environment();
    let args = json!({});
    let ctx = context::<()>(&args, &setup, &env, None);
    let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
    let observations = Rc::clone(&seen);
    let callback = ToolRenderResult::<Vec<String>, ()>::Typed(Rc::new(move |result, _, _, _| {
        observations.borrow_mut().push(result.details.cloned());
        Ok(empty())
    }));
    let count = Cell::new(0);
    let options = ToolRenderResultOptions {
        expanded: false,
        is_partial: false,
    };
    let payload = json!(["custom", "shape"]);
    for details in [None, Some(&serde_json::Value::Null), Some(&payload)] {
        callback
            .invoke_raw((&raw(details), options), &env.theme, &ctx, |value| {
                count.set(count.get() + 1);
                if value.is_null() {
                    Ok(vec!["null".into()])
                } else {
                    Ok(serde_json::from_value(value.clone()).unwrap())
                }
            })
            .unwrap();
    }
    assert_eq!(count.get(), 2);
    assert_eq!(
        *seen.borrow(),
        [
            None,
            Some(vec!["null".into()]),
            Some(vec!["custom".into(), "shape".into()])
        ]
    );
    assert!(
        callback
            .invoke_raw((&raw(Some(&payload)), options), &env.theme, &ctx, |_| Err(
                failure()
            ))
            .is_err()
    );
    assert_eq!(seen.borrow().len(), 3);
}

use crate::renderer_projection_support::definition;
use maestro_tools::{ToolRenderShell, ToolRendererBinding, ToolRendererState};
use std::cell::RefCell;

#[test]
fn renderer_binding_keeps_same_type_state_across_slots() {
    let (env, setup) = environment();
    let args = json!({});
    let row = context::<ToolRendererState>(&args, &setup, &env, None);
    let aliases = Rc::new(RefCell::new(Vec::new()));
    let call_aliases = Rc::clone(&aliases);
    let result_aliases = Rc::clone(&aliases);
    let mut tool = definition::<u32, Vec<String>>();
    tool.render_call = Some(Rc::new(move |_, _, row| {
        *row.state.borrow_mut() = 73;
        call_aliases.borrow_mut().push(Rc::clone(&row.state));
        // Reentry into the borrowed owner must not collide with a retained map borrow.
        let other = row.renderer_state().unwrap().state::<serde_json::Value>();
        assert_eq!(*other.borrow(), json!({}));
        Ok(empty())
    }));
    tool.render_result = Some(ToolRenderResult::Typed(Rc::new(
        move |result, _, _, row| {
            assert_eq!(result.details.unwrap(), &["custom"]);
            assert_eq!(*row.state.borrow(), 73);
            result_aliases.borrow_mut().push(Rc::clone(&row.state));
            Ok(empty())
        },
    )));
    let first = ToolRendererBinding::from_definition(&tool);
    let second = ToolRendererBinding::from_definition(&tool);
    assert_eq!(first.render_shell, Some(ToolRenderShell::SelfRendered));
    first.render_call.unwrap()(Some(&args), &env.theme, &row).unwrap();
    second.render_result.unwrap()(
        &raw(Some(&json!(["custom"]))),
        ToolRenderResultOptions {
            expanded: false,
            is_partial: false,
        },
        &env.theme,
        &row,
    )
    .unwrap();
    let aliases = aliases.borrow();
    assert!(Rc::ptr_eq(&aliases[0], &aliases[1]));
}
#[test]
fn renderer_binding_coexists_with_builtin_private_state() {
    let (env, setup) = environment();
    let args = json!({"command":"echo observed"});
    let row = context::<ToolRendererState>(&args, &setup, &env, None);
    let other = context::<ToolRendererState>(&args, &setup, &env, None);
    let tool = maestro_tools::ToolDef::Bash(maestro_tools::create_bash_tool_definition::<()>(
        "/work".into(),
        "/unused".into(),
        maestro_tools::BashToolOptions::default(),
    ));
    let binding = tool.renderer_binding().unwrap();
    let call = binding.render_call.unwrap()(Some(&args), &env.theme, &row).unwrap();
    assert!(call.render(80).join("\n").contains("echo observed"));
    let native = row
        .state
        .borrow_mut()
        .state::<maestro_tools::BashRenderState>();
    let mut custom = definition::<u32, serde_json::Value>();
    custom.render_call = Some(Rc::new(|_, _, row| {
        *row.state.borrow_mut() += 1;
        Ok(empty())
    }));
    let call = ToolRendererBinding::from_definition(&custom)
        .render_call
        .unwrap();
    call(None, &env.theme, &row).unwrap();
    call(None, &env.theme, &row).unwrap();
    call(None, &env.theme, &other).unwrap();
    assert_eq!(*row.state.borrow_mut().state::<u32>().borrow(), 2);
    assert_eq!(*other.state.borrow_mut().state::<u32>().borrow(), 1);
    assert!(Rc::ptr_eq(
        &native,
        &row.state
            .borrow_mut()
            .state::<maestro_tools::BashRenderState>()
    ));
    assert!(!Rc::ptr_eq(
        &native,
        &other
            .state
            .borrow_mut()
            .state::<maestro_tools::BashRenderState>()
    ));
    if let maestro_tools::ToolDef::Bash(mut tool) = tool {
        tool.render_result = Some(ToolRenderResult::Typed(Rc::new(|_, _, _, _| Ok(empty()))));
        assert!(
            maestro_tools::ToolDef::Bash(tool)
                .renderer_binding()
                .is_err()
        );
    }
}
#[test]
fn renderer_binding_keeps_context_and_previous_slot_handle() {
    let (env, mut setup) = environment();
    setup.expanded = true;
    setup.is_error = true;
    setup.phase.args_complete = true;
    let args = json!({"distinct":3});
    let previous = empty();
    let mut row = context::<ToolRendererState>(&args, &setup, &env, Some(Rc::clone(&previous)));
    row.tool_call_id = "distinct-id".into();
    let invalidations = Rc::new(Cell::new(0));
    let counter = Rc::clone(&invalidations);
    row.invalidate = Rc::new(move || counter.set(counter.get() + 1));
    let mut tool = definition::<u32, serde_json::Value>();
    let check = move |ctx: &maestro_tools::ToolRenderContext<'_, u32>| {
        assert_eq!(ctx.args, Some(&json!({"distinct":3})));
        assert_eq!(ctx.tool_call_id, "distinct-id");
        assert_eq!(ctx.cwd, "/work");
        assert_eq!(ctx.environment.readme_path, "/opt/maestro/README.md");
        assert!(ctx.phase.args_complete && !ctx.phase.execution_started && !ctx.phase.is_partial);
        assert!(ctx.expanded && ctx.is_error && ctx.show_images);
        assert!(ctx.renderer_state().is_some());
        (ctx.invalidate)();
        ctx.last_component.clone().unwrap()
    };
    tool.render_call = Some(Rc::new(move |_, _, ctx| Ok(check(ctx))));
    let binding = ToolRendererBinding::from_definition(&tool);
    let returned = binding.render_call.unwrap()(Some(&args), &env.theme, &row).unwrap();
    assert!(Rc::ptr_eq(&previous, &returned));
    assert_eq!(invalidations.get(), 1);
    let projected = row.with_state(Rc::new(RefCell::new(4u32)));
    assert!(Rc::ptr_eq(&projected.environment, &row.environment));
    assert!(std::ptr::eq(projected.args.unwrap(), row.args.unwrap()));
}
#[test]
fn renderer_binding_drop_releases_captures_and_row_state() {
    let token = Rc::new(());
    let weak = Rc::downgrade(&token);
    let mut tool = definition::<u32, serde_json::Value>();
    tool.render_call = Some(Rc::new(move |_, _, _| {
        assert_eq!(Rc::strong_count(&token), 1);
        Ok(empty())
    }));
    let executable = std::sync::Arc::new(());
    let executable_weak = std::sync::Arc::downgrade(&executable);
    tool.execute = std::sync::Arc::new(move |_, (), _, _, _| {
        assert_eq!(std::sync::Arc::strong_count(&executable), 1);
        Box::pin(async { Err(failure()) })
    });
    let binding = ToolRendererBinding::from_definition(&tool);
    drop(tool);
    assert!(executable_weak.upgrade().is_none());
    assert!(weak.upgrade().is_some());
    drop(binding);
    assert!(weak.upgrade().is_none());
    let row = ToolRendererState::default();
    let state = row.state::<u32>();
    let state_weak = Rc::downgrade(&state);
    drop(row);
    assert!(state_weak.upgrade().is_some());
    drop(state);
    assert!(state_weak.upgrade().is_none());
}

#[test]
fn renderer_binding_can_wrap_a_builtin_callback() {
    crate::read_support::run(async {
        let dir = crate::read_support::TempDir::new();
        dir.file("file", b"alpha\n");
        let env = Env::with(
            Rc::new(crate::edit_render_support::palette(0)),
            Rc::new(crate::display_support::NoLanguages),
        );
        let setup = Setup::from(
            &json!({"readme":"/docs/README.md","bindings":[],"cwd":dir.text(),"argsComplete":true}),
        );
        let args = json!({"path":"file","oldText":"alpha","newText":"beta"});
        let mut row = context::<ToolRendererState>(&args, &setup, &env, None);
        let custom = wrapped_edit(&dir.text());
        let binding = ToolRendererBinding::from_definition(&custom);
        let render_call = binding.render_call.unwrap();
        let call = render_call(Some(&args), &env.theme, &row).unwrap();
        assert_eq!(env.runtime.futures(), 1);
        for future in env.runtime.take_futures() {
            future.await.unwrap();
        }
        row.last_component = Some(Rc::clone(&call));
        assert!(Rc::ptr_eq(
            &call,
            &render_call(Some(&args), &env.theme, &row).unwrap()
        ));
        let before = call.render(80);
        assert!(before.join("\n").contains("beta"));
        row.last_component = None;
        let output = binding.render_result.unwrap()(
            &raw(Some(&json!(["+1 persisted"]))),
            ToolRenderResultOptions {
                expanded: false,
                is_partial: false,
            },
            &env.theme,
            &row,
        )
        .unwrap();
        assert!(output.render(80).is_empty());
        assert!(call.render(80).join("\n").contains("persisted"));
        let state = row
            .state
            .borrow_mut()
            .state::<maestro_tools::EditRenderState>();
        let reused = (maestro_tools::create_edit_tool_definition::<()>(
            dir.text(),
            maestro_tools::EditToolOptions::default(),
        )
        .render_call
        .unwrap())(Some(&args), &env.theme, &row.with_state(state))
        .unwrap();
        assert!(Rc::ptr_eq(&call, &reused));
        assert_eq!(env.runtime.futures(), 0);
        assert_eq!(std::fs::read(dir.0.join("file")).unwrap(), b"alpha\n");
    });
}
#[test]
fn renderer_projection_wrong_selected_fields_use_absent_fallback() {
    let env = crate::renderer_projection_support::environment();
    for (tool, fields) in [
        (
            "read",
            vec![
                "truncated",
                "firstLineExceedsLimit",
                "truncatedBy",
                "maxBytes",
                "maxLines",
                "outputLines",
                "totalLines",
            ],
        ),
        (
            "bash",
            vec![
                "truncated",
                "truncatedBy",
                "maxBytes",
                "outputLines",
                "totalLines",
                "fullOutputPath",
            ],
        ),
        (
            "grep",
            vec![
                "truncated",
                "maxBytes",
                "matchLimitReached",
                "linesTruncated",
            ],
        ),
        ("find", vec!["truncated", "maxBytes", "resultLimitReached"]),
        ("ls", vec!["truncated", "maxBytes", "entryLimitReached"]),
        ("edit", vec!["diff", "firstChangedLine"]),
    ] {
        for field in fields {
            check_wrong_field(tool, field, &env);
        }
    }
}

/// Compare malformed selected types with the corresponding absent-field control.
fn check_wrong_field(tool: &str, field: &str, env: &Env) {
    use crate::renderer_projection_support::{Input, render};
    let nested = [
        "truncated",
        "firstLineExceedsLimit",
        "truncatedBy",
        "maxBytes",
        "maxLines",
        "outputLines",
        "totalLines",
    ]
    .contains(&field);
    let mut base = json!({"tool":tool,"args":{"path":"notes.txt"},"details":{},"options":{"expanded":false,"isPartial":false},"isError":false,"content":[{"type":"text","text":"body"}]});
    if nested {
        base["details"]["truncation"] =
            json!({"truncated":true,"truncatedBy":"lines","outputLines":2,"totalLines":3});
    }
    if field == "firstChangedLine" {
        base["details"]["diff"] = json!("+1 after");
    }
    let target = if nested {
        &mut base["details"]["truncation"]
    } else {
        &mut base["details"]
    };
    target.as_object_mut().unwrap().shift_remove(field);
    let control: Input = serde_json::from_value(base.clone()).unwrap();
    let expected = render(&control, env);
    for wrong in [json!({"wrong":true}), json!(["wrong"])] {
        let target = if nested {
            &mut base["details"]["truncation"]
        } else {
            &mut base["details"]
        };
        target[field] = wrong;
        let input: Input = serde_json::from_value(base.clone()).unwrap();
        assert_eq!(render(&input, env), expected, "{tool} {field}");
    }
}

/// Arbitrary custom details/state delegate to the existing native edit state and callback.
fn wrapped_edit(cwd: &str) -> maestro_tools::ToolDefinition<(), (), Vec<String>, u32> {
    let native = maestro_tools::create_edit_tool_definition::<()>(
        cwd.to_owned(),
        maestro_tools::EditToolOptions::default(),
    );
    let call = native.render_call.unwrap();
    let ToolRenderResult::Raw(result) = native.render_result.unwrap() else {
        panic!("factory result is raw")
    };
    let mut custom = definition::<u32, Vec<String>>();
    custom.render_call = Some(Rc::new(move |args, theme, row| {
        *row.state.borrow_mut() = 17;
        let state = row
            .renderer_state()
            .unwrap()
            .state::<maestro_tools::EditRenderState>();
        call(args, theme, &row.with_state(state))
    }));
    custom.render_result = Some(ToolRenderResult::Typed(Rc::new(
        move |value, options, theme, row| {
            assert_eq!(*row.state.borrow(), 17);
            let state = row
                .renderer_state()
                .unwrap()
                .state::<maestro_tools::EditRenderState>();
            ToolRenderResult::<Vec<String>, maestro_tools::EditRenderState>::Raw(Rc::clone(&result))
                .invoke_native((value, options), theme, &row.with_state(state), |details| {
                    Ok(json!({"diff":details[0],"firstChangedLine":1.25}))
                })
        },
    )));
    custom
}

thread_local! {
    /// Only this test transaction sets this thread's row owner.
    static DEFAULT_ROW: RefCell<Option<ToolRendererState>> = const { RefCell::new(None) };
}
/// A custom initializer reading the same map before publication.
struct MapReader(u32);
impl Default for MapReader {
    fn default() -> Self {
        let owner = DEFAULT_ROW.with(|row| row.borrow().clone().unwrap());
        Self(*owner.state::<u32>().borrow())
    }
}
#[test]
fn renderer_state_defaults_can_read_the_same_row_map() {
    let owner = ToolRendererState::default();
    *owner.state::<u32>().borrow_mut() = 73;
    DEFAULT_ROW.with(|row| *row.borrow_mut() = Some(owner.clone()));
    let state = owner.state::<MapReader>();
    DEFAULT_ROW.with(|row| row.borrow_mut().take());
    assert_eq!(state.borrow().0, 73);
    assert!(Rc::ptr_eq(&state, &owner.state::<MapReader>()));
}
