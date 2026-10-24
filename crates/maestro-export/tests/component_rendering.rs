//! Standard component renderer behavior.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
#[test]
fn result_padding_keeps_red_one_and_plain_two() {
    component_support::check("result_padding_keeps_red_one_and_plain_two");
}
use crate::component_support;
#[test]
fn width_defaults_only_when_omitted() {
    component_support::check("width_defaults_only_when_omitted");
}
#[test]
fn call_and_result_forward_inputs_and_flags() {
    component_support::check("call_and_result_forward_inputs_and_flags");
}
#[test]
fn arguments_preserve_absence_null_and_replacement() {
    component_support::check("arguments_preserve_absence_null_and_replacement");
}
#[test]
fn state_and_component_slots_are_independent_per_call() {
    component_support::check("state_and_component_slots_are_independent_per_call");
}
#[test]
fn lookup_is_live_and_absent_callbacks_return_none() {
    component_support::check("lookup_is_live_and_absent_callbacks_return_none");
}
#[test]
fn call_failures_preserve_prior_mutations() {
    component_support::check("call_failures_preserve_prior_mutations");
}
#[test]
fn result_failures_stop_at_the_reached_phase() {
    component_support::check("result_failures_stop_at_the_reached_phase");
}
#[test]
fn result_output_omission_depends_on_converted_html() {
    component_support::check("result_output_omission_depends_on_converted_html");
}
#[test]
fn result_edge_trimming_uses_sgr_and_shared_whitespace() {
    component_support::check("result_edge_trimming_uses_sgr_and_shared_whitespace");
}
#[test]
fn callback_reentry_does_not_hold_row_borrows() {
    component_support::check("callback_reentry_does_not_hold_row_borrows");
}

#[test]
fn typed_definitions_keep_custom_state_and_details() {
    use maestro_export::{
        ToolHtmlRenderer, ToolHtmlRendererDeps, ToolHtmlResultInput, create_tool_html_renderer,
    };
    use serde_json::json;
    use std::{cell::RefCell, rc::Rc};
    let states = Rc::new(RefCell::new(Vec::new()));
    let tool = typed_definition(&states);
    let theme = component_support::theme();
    let tool = maestro_tools::ToolRendererBinding::from_definition(&tool);
    let renderer = create_tool_html_renderer(ToolHtmlRendererDeps {
        get_tool_definition: |_: &str| Ok(Some(tool.clone())),
        theme: maestro_theme::LiveTheme::from_theme(Rc::new(theme)),
        cwd: "/work",
        width: None,
        environment: component_support::environment(),
    });
    let content = json!([]);
    let details = json!({"label":"typed"});
    let output = renderer
        .render_result(
            "id",
            "custom",
            ToolHtmlResultInput {
                content: Some(&content),
                details: Some(&details),
                is_error: false,
            },
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        output.collapsed.as_deref(),
        Some("<div class=\"ansi-line\">typed:1:false</div>")
    );
    assert_eq!(
        output.expanded.as_deref(),
        Some("<div class=\"ansi-line\">typed:2:true</div>")
    );
    let states = states.borrow();
    assert!(Rc::ptr_eq(&states[0], &states[1]));
    assert_eq!(states[0].borrow().count, 2);
    let wrong = json!({"label":false});
    assert!(
        renderer
            .render_result(
                "id",
                "custom",
                ToolHtmlResultInput {
                    content: Some(&content),
                    details: Some(&wrong),
                    is_error: false,
                }
            )
            .unwrap()
            .is_none()
    );
    assert_eq!(states[0].borrow().count, 2);
}

use maestro_tui::{Component, interface::ComponentHandle};
use serde::Deserialize;
use std::{cell::RefCell, rc::Rc};
#[derive(Default)]
struct State {
    count: usize,
}
#[derive(Deserialize)]
struct Details {
    label: String,
}
struct Parameters;

type SavedStates = Rc<RefCell<Vec<Rc<RefCell<State>>>>>;
fn typed_definition(
    states: &SavedStates,
) -> maestro_tools::types::local::ToolDefinition<u32, Parameters, Details, State> {
    let saved = Rc::clone(states);
    let mut tool = component_support::definition::<u32, Parameters, Details, State>();
    tool.render_result = Some(
        maestro_tools::ToolRenderResult::<_, _, maestro_models::Local>::Typed(Rc::new(
            move |result, options, _, ctx| {
                saved.borrow_mut().push(Rc::clone(&ctx.state));
                let mut state = ctx.state.borrow_mut();
                state.count += 1;
                let text = format!(
                    "{}:{}:{}",
                    result.details.unwrap().label,
                    state.count,
                    options.expanded
                );
                Ok(Rc::new(component_support::Line(text)) as ComponentHandle)
            },
        )),
    );
    tool
}

#[test]
fn nested_component_failures_fall_back_and_stop_later_phases() {
    for padded in [false, true] {
        for failing_phase in 0..3 {
            assert_nested_failure(padded, failing_phase);
        }
    }
}

fn nest(child: ComponentHandle, padded: bool) -> ComponentHandle {
    use maestro_tui::{Container, components::Box as PaddedBox};
    if padded {
        let parent = PaddedBox::default();
        parent.add_child(child);
        Rc::new(parent)
    } else {
        let parent = Container::new();
        parent.add_child(child);
        Rc::new(parent)
    }
}

fn assert_nested_failure(padded: bool, failing_phase: usize) {
    use maestro_export::{
        ToolHtmlRenderer, ToolHtmlRendererDeps, ToolHtmlResultInput, create_tool_html_renderer,
    };
    use std::cell::Cell;
    let reached = Rc::new(Cell::new(0));
    let mut tool: maestro_tools::types::local::ToolDefinition = component_support::definition();
    let seen = Rc::clone(&reached);
    tool.render_call = Some(Rc::new(move |_, _, _| {
        seen.set(seen.get() + 1);
        Ok(nest(Rc::new(Fallible(failing_phase == 0)), padded))
    }));
    let seen = Rc::clone(&reached);
    tool.render_result = Some(
        maestro_tools::ToolRenderResult::<_, _, maestro_models::Local>::Typed(Rc::new(
            move |_, options, _, _| {
                seen.set(seen.get() + 1);
                let phase = if options.expanded { 2 } else { 1 };
                Ok(nest(Rc::new(Fallible(failing_phase == phase)), padded))
            },
        )),
    );
    let tool = Rc::new(tool);
    let theme = component_support::theme();
    let renderer = create_tool_html_renderer(ToolHtmlRendererDeps {
        get_tool_definition: |_: &str| {
            Ok(Some(maestro_tools::ToolRendererBinding::from_definition(
                &tool,
            )))
        },
        theme: maestro_theme::LiveTheme::from_theme(Rc::new(theme)),
        cwd: "/work",
        width: None,
        environment: component_support::environment(),
    });
    if failing_phase == 0 {
        assert!(
            renderer
                .render_call("id", "custom", None)
                .unwrap()
                .is_none()
        );
        assert_eq!(reached.get(), 1);
    } else {
        assert!(
            renderer
                .render_result(
                    "id",
                    "custom",
                    ToolHtmlResultInput {
                        content: None,
                        details: None,
                        is_error: false,
                    }
                )
                .unwrap()
                .is_none()
        );
        assert_eq!(reached.get(), failing_phase);
    }
}

struct Fallible(bool);
impl Component for Fallible {
    fn render(&self, _: usize) -> Vec<String> {
        Vec::new()
    }
    fn try_render(&self, _: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        if self.0 {
            Err(std::io::Error::other("nested component failure").into())
        } else {
            Ok(vec!["rendered".into()])
        }
    }
}

/// Details whose decoding creates a distinguishable identity shared by both render phases.
struct Decoded {
    token: Rc<()>,
}
thread_local! {
    static DECODES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
impl<'de> Deserialize<'de> for Decoded {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        let _ = serde_json::Value::deserialize(decoder)?;
        DECODES.with(|count| count.set(count.get() + 1));
        Ok(Self { token: Rc::new(()) })
    }
}
#[test]
fn export_typed_details_decode_once_and_share_both_phases() {
    use maestro_export::{
        ToolHtmlRenderer, ToolHtmlRendererDeps, ToolHtmlResultInput, create_tool_html_renderer,
    };
    DECODES.with(|count| count.set(0));
    let tokens = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&tokens);
    let mut tool = component_support::definition::<(), (), Decoded, serde_json::Value>();
    tool.render_result = Some(
        maestro_tools::ToolRenderResult::<_, _, maestro_models::Local>::Typed(Rc::new(
            move |result, options, _, _| {
                seen.borrow_mut()
                    .push(Rc::clone(&result.details.unwrap().token));
                Ok(Rc::new(component_support::Line(format!(
                    "phase:{}",
                    options.expanded
                ))))
            },
        )),
    );
    let tool = Rc::new(tool);
    let theme = component_support::theme();
    let renderer = create_tool_html_renderer(ToolHtmlRendererDeps {
        get_tool_definition: |_: &str| {
            Ok(Some(maestro_tools::ToolRendererBinding::from_definition(
                &tool,
            )))
        },
        theme: maestro_theme::LiveTheme::from_theme(Rc::new(theme)),
        cwd: "/work",
        width: None,
        environment: component_support::environment(),
    });
    let details = serde_json::json!({"arbitrary":true});
    let output = renderer
        .render_result(
            "id",
            "custom",
            ToolHtmlResultInput {
                content: None,
                details: Some(&details),
                is_error: false,
            },
        )
        .unwrap()
        .unwrap();
    assert!(output.collapsed.unwrap().contains("phase:false"));
    assert!(output.expanded.unwrap().contains("phase:true"));
    assert_eq!(DECODES.with(std::cell::Cell::get), 1);
    let tokens = tokens.borrow();
    assert!(Rc::ptr_eq(&tokens[0], &tokens[1]));
}

#[test]
fn export_raw_details_do_not_decode_typed_records() {
    use maestro_export::{
        ToolHtmlRenderer, ToolHtmlRendererDeps, ToolHtmlResultInput, create_tool_html_renderer,
    };
    use maestro_theme::{ColorMode, ColorValue, Theme, ThemeColor, ThemeOptions};
    let theme = Rc::new(
        Theme::new(
            [ThemeColor::ToolOutput, ThemeColor::Warning]
                .into_iter()
                .map(|color| (color, ColorValue::Index(2))),
            [],
            ColorMode::Color256,
            ThemeOptions::default(),
        )
        .unwrap(),
    );
    let environment = component_support::environment();
    let tool = Rc::new(maestro_tools::create_read_tool_definition::<()>(
        "/work".into(),
        maestro_tools::ReadToolOptions::default(),
    ));
    let renderer = create_tool_html_renderer(ToolHtmlRendererDeps {
        get_tool_definition: |_: &str| {
            Ok(Some(maestro_tools::ToolRendererBinding::from_definition(
                &tool,
            )))
        },
        theme: maestro_theme::LiveTheme::from_theme(theme),
        cwd: "/work",
        width: Some(100),
        environment,
    });
    renderer
        .render_call("id", "read", Some(&serde_json::json!({"path":"notes.txt"})))
        .unwrap();
    let details = serde_json::json!({"truncation":{"truncated":true,"truncatedBy":"lines","outputLines":2,"totalLines":3,"maxLines":2}});
    let output = renderer
        .render_result(
            "id",
            "read",
            ToolHtmlResultInput {
                content: None,
                details: Some(&details),
                is_error: false,
            },
        )
        .unwrap()
        .unwrap();
    assert!(
        output
            .expanded
            .unwrap()
            .contains("Truncated: showing 2 of 3 lines (2 line limit)")
    );
}

#[test]
fn export_wrapped_native_edit_keeps_shared_row_state() {
    use maestro_export::{
        ToolHtmlRenderer, ToolHtmlRendererDeps, ToolHtmlResultInput, create_tool_html_renderer,
    };
    let states = Rc::new(RefCell::new(Vec::new()));
    let tool = wrapped_edit(&states);
    let tool = Rc::new(tool);
    let (theme, environment) = native_environment();
    let renderer = create_tool_html_renderer(ToolHtmlRendererDeps {
        get_tool_definition: |_: &str| {
            Ok(Some(maestro_tools::ToolRendererBinding::from_definition(
                &tool,
            )))
        },
        theme: maestro_theme::LiveTheme::from_theme(theme),
        cwd: "/work",
        width: Some(80),
        environment: Rc::clone(&environment),
    });
    let args = serde_json::json!({"path":"notes.txt"});
    renderer
        .render_call("first", "edit", Some(&args))
        .unwrap()
        .unwrap();
    let result = |id, details| {
        renderer
            .render_result(
                id,
                "edit",
                ToolHtmlResultInput {
                    content: None,
                    details: Some(details),
                    is_error: false,
                },
            )
            .unwrap()
            .unwrap()
    };
    let details = serde_json::json!({"diff":"+1 persisted","firstChangedLine":1.25});
    let output = result("first", &details);
    assert_eq!(output.expanded.as_deref(), Some(""));
    let fractional = serde_json::json!({"diff":"+1 persisted","firstChangedLine":1.75});
    assert_eq!(result("first", &fractional).expanded.as_deref(), Some(""));
    let settled = renderer
        .render_call("first", "edit", Some(&args))
        .unwrap()
        .unwrap();
    assert!(settled.contains("persisted"));
    let other = renderer
        .render_call("second", "edit", Some(&args))
        .unwrap()
        .unwrap();
    assert!(!other.contains("persisted"));
    result("second", &details);
    let states = states.borrow();
    assert!(Rc::ptr_eq(&states[0], &states[1]));
    assert!(Rc::ptr_eq(&states[0], &states[3]));
    assert!(!Rc::ptr_eq(&states[0], &states[4]));
}

/// A custom state wrapper delegates both slots to the native edit state owner.
fn wrapped_edit(
    states: &Rc<RefCell<Vec<Rc<RefCell<maestro_tools::EditRenderState>>>>>,
) -> maestro_tools::types::local::ToolDefinition<(), (), serde_json::Value, u32> {
    use maestro_tools::{EditRenderState, ToolRenderResult};
    let tool = maestro_tools::create_edit_tool_definition::<()>(
        "/work".into(),
        maestro_tools::EditToolOptions::default(),
    );
    let native_call = tool.render_call.unwrap();
    let ToolRenderResult::Raw(native_result) = tool.render_result.unwrap() else {
        panic!("raw native callback")
    };
    let mut tool = component_support::definition::<(), (), serde_json::Value, u32>();
    let seen = Rc::clone(states);
    let retained = Rc::new(RefCell::new(None));
    let call_slot = Rc::clone(&retained);
    tool.render_call = Some(Rc::new(move |args, theme, row| {
        *row.state.borrow_mut() += 1;
        let state = row.renderer_state().unwrap().state::<EditRenderState>();
        let call = native_call(args, theme, &row.with_state(state))?;
        call_slot.replace(Some(Rc::clone(&call)));
        Ok(call)
    }));
    tool.render_result = Some(ToolRenderResult::<_, _, maestro_models::Local>::Raw(
        Rc::new(move |result, options, theme, row| {
            assert_eq!(*row.state.borrow(), 1);
            let state = row.renderer_state().unwrap().state::<EditRenderState>();
            seen.borrow_mut().push(Rc::clone(&state));
            let fractional_change = result
                .details
                .and_then(|d| d.get("firstChangedLine"))
                .and_then(serde_json::Value::as_f64)
                == Some(1.75);
            let alternate = maestro_theme::LiveTheme::from_theme(native_palette(3));
            let theme = if fractional_change { &alternate } else { theme };
            let output = native_result(result, options, theme, &row.with_state(state))?;
            if fractional_change {
                let call = retained.borrow().as_ref().unwrap().clone();
                let html = maestro_export::ansi_lines_to_html(&call.render(80));
                assert!(html.contains("#808000"));
                assert!(html.contains("persisted"));
            }
            Ok(output)
        }),
    ));
    tool
}

/// Publish colors consumed by real native edit callbacks.
fn native_environment() -> (
    Rc<maestro_theme::Theme>,
    Rc<maestro_tools::ToolRenderEnvironment>,
) {
    let theme = native_palette(2);
    let state = maestro_theme::ThemeState::new(maestro_theme::ThemeDirectories {
        themes_dir: String::new(),
        custom_themes_dir: String::new(),
    });
    state.set_theme_instance(Rc::clone(&theme)).unwrap();
    let mut environment = component_support::environment();
    Rc::get_mut(&mut environment).unwrap().live_theme = state.theme();
    (theme, environment)
}

/// Controlled foreground palette for observing callback rebuilds.
fn native_palette(color: u8) -> Rc<maestro_theme::Theme> {
    Rc::new(
        maestro_theme::Theme::new(
            [
                "toolTitle",
                "accent",
                "toolOutput",
                "toolDiffAdded",
                "toolDiffRemoved",
                "toolDiffContext",
                "error",
            ]
            .into_iter()
            .map(|key| {
                (
                    maestro_theme::ThemeColor::Named(key.into()),
                    maestro_theme::ColorValue::Index(color),
                )
            }),
            ["toolPendingBg", "toolSuccessBg", "toolErrorBg"]
                .into_iter()
                .map(|key| {
                    (
                        maestro_theme::ThemeBg::Named(key.into()),
                        maestro_theme::ColorValue::Index(0),
                    )
                }),
            maestro_theme::ColorMode::Color256,
            maestro_theme::ThemeOptions::default(),
        )
        .unwrap(),
    )
}

#[test]
fn export_mixed_bindings_require_only_selected_decoder() {
    use component_support::bindings::{native_environment, registry};
    use maestro_export::{ToolHtmlRendererDeps, create_tool_html_renderer};
    let (theme, environment) = native_environment();
    let bindings = registry();
    let renderer = create_tool_html_renderer(ToolHtmlRendererDeps {
        get_tool_definition: |name: &str| Ok(bindings.get(name).cloned()),
        theme: maestro_theme::LiveTheme::from_theme(theme),
        cwd: "/work",
        width: Some(80),
        environment,
    });
    check_native_registry(&renderer);
    check_custom_registry(&renderer);
}

fn check_native_registry(renderer: &dyn maestro_export::ToolHtmlRenderer) {
    use component_support::{bindings::corpus, result_value};
    use maestro_export::ToolHtmlResultInput;
    for case in corpus().native {
        assert_eq!(
            case.test,
            "export_mixed_bindings_require_only_selected_decoder"
        );
        let input = case.input;
        let call = renderer
            .render_call(&input.name, &input.name, Some(&input.args))
            .unwrap()
            .unwrap();
        let result = renderer
            .render_result(
                &input.name,
                &input.name,
                ToolHtmlResultInput {
                    content: Some(&input.content),
                    details: Some(&input.details),
                    is_error: false,
                },
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            serde_json::json!({"call":call,"result":result_value(result)}),
            case.expected
        );
    }
}

fn check_custom_registry(renderer: &dyn maestro_export::ToolHtmlRenderer) {
    use maestro_export::ToolHtmlResultInput;
    for (name, details, expected) in [
        (
            "custom_text",
            serde_json::json!({"label":"detail"}),
            "custom_text:detail:true",
        ),
        (
            "custom_count",
            serde_json::json!({"value":42}),
            "custom_count:42:true",
        ),
    ] {
        let result = renderer
            .render_result(
                name,
                name,
                ToolHtmlResultInput {
                    content: None,
                    details: Some(&details),
                    is_error: false,
                },
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            result.expanded.unwrap(),
            format!("<div class=\"ansi-line\">{expected}</div>")
        );
    }
    let raw = renderer
        .render_result(
            "raw",
            "raw",
            ToolHtmlResultInput {
                content: None,
                details: Some(&serde_json::json!({"not":"typed"})),
                is_error: false,
            },
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        raw.expanded.as_deref(),
        Some("<div class=\"ansi-line\">raw</div>")
    );
}

#[test]
fn export_raw_bindings_borrow_details_in_both_phases() {
    use component_support::{bindings::corpus, result_value};
    use maestro_export::{
        ToolHtmlRenderer, ToolHtmlRendererDeps, ToolHtmlResultInput, create_tool_html_renderer,
    };
    use maestro_tools::ToolRendererBinding;
    use serde_json::json;
    for case in corpus().mixed.into_iter().take(7) {
        assert_eq!(
            case.test,
            "export_raw_bindings_borrow_details_in_both_phases"
        );
        let input = case.input;
        for typed_constructor in [false, true] {
            let events = Rc::new(RefCell::new(Vec::new()));
            let tool = raw_observer(
                input.details.as_ref().map(std::ptr::from_ref),
                events.clone(),
            );
            let binding = if typed_constructor {
                ToolRendererBinding::from_definition(&tool)
            } else {
                ToolRendererBinding::from_raw(&tool).unwrap()
            };
            let theme = component_support::theme();
            let renderer = create_tool_html_renderer(ToolHtmlRendererDeps {
                get_tool_definition: |_: &str| Ok(Some(binding.clone())),
                theme: maestro_theme::LiveTheme::from_theme(Rc::new(theme)),
                cwd: "/work",
                width: None,
                environment: component_support::environment(),
            });
            renderer
                .render_call("id", "custom", Some(&json!({"tag":input.case})))
                .unwrap();
            let content = serde_json::to_value(&input.content).unwrap();
            let result = renderer
                .render_result(
                    "id",
                    "custom",
                    ToolHtmlResultInput {
                        content: Some(&content),
                        details: input.details.as_ref(),
                        is_error: true,
                    },
                )
                .unwrap()
                .unwrap();
            assert_eq!(
                json!({"output":result_value(result),"events":*events.borrow()}),
                case.expected
            );
        }
    }
}

#[test]
fn export_prepared_results_survive_reentry_and_live_lookup() {
    use component_support::{bindings::corpus, result_value};
    use maestro_export::ToolHtmlResultInput;
    use serde_json::json;
    let case = corpus().mixed.remove(7);
    assert_eq!(
        case.test,
        "export_prepared_results_survive_reentry_and_live_lookup"
    );
    assert_eq!(case.input.case, "same-id-reentry");
    let (renderer, events) = reentrant_renderer(case.input.inner_details.clone().unwrap());
    renderer
        .render_call("id", "outer", Some(&json!({"version":"first"})))
        .unwrap();
    let result = renderer
        .render_result(
            "id",
            "outer",
            ToolHtmlResultInput {
                content: None,
                details: case.input.outer_details.as_ref(),
                is_error: false,
            },
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        json!({"output":result_value(result),"events":*events.borrow()}),
        case.expected
    );
    let result = renderer
        .render_result(
            "id",
            "outer",
            ToolHtmlResultInput {
                content: None,
                details: None,
                is_error: false,
            },
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        result.expanded.as_deref(),
        Some("<div class=\"ansi-line\">replacement</div>")
    );
    assert_eq!(
        events.borrow().last().unwrap(),
        &json!({"lookup":"outer","version":"replacement"})
    );
}

#[test]
fn export_binding_changes_keep_native_state_per_call() {
    use component_support::bindings::{corpus, native_environment};
    use maestro_export::{ToolHtmlRendererDeps, create_tool_html_renderer};
    use maestro_tools::{ToolRenderResult, ToolRendererBinding};
    let case = corpus().mixed.remove(8);
    assert_eq!(
        case.test,
        "export_binding_changes_keep_native_state_per_call"
    );
    assert_eq!(case.input.case, "native-edit-replaced");
    let native = maestro_tools::create_edit_tool_definition::<()>(
        "/work".into(),
        maestro_tools::EditToolOptions::default(),
    );
    let native_binding = ToolRendererBinding::from_raw(&native).unwrap();
    let ToolRenderResult::Raw(callback) = native.render_result.unwrap() else {
        panic!("raw callback")
    };
    let states = Rc::new(RefCell::new(Vec::new()));
    let custom = edit_result_wrapper(callback, &states);
    let result_only = std::cell::Cell::new(false);
    let (theme, environment) = native_environment();
    let renderer = create_tool_html_renderer(ToolHtmlRendererDeps {
        get_tool_definition: |_: &str| {
            Ok(Some(if result_only.get() {
                custom.clone()
            } else {
                native_binding.clone()
            }))
        },
        theme: maestro_theme::LiveTheme::from_theme(theme),
        cwd: "/work",
        width: Some(80),
        environment,
    });
    check_edit_replacement(&renderer, &result_only, &case, &states);
}

fn raw_observer(
    details: Option<*const serde_json::Value>,
    observed: Rc<RefCell<Vec<serde_json::Value>>>,
) -> maestro_tools::types::local::ToolDefinition<
    (),
    (),
    component_support::bindings::NeverDecode,
    u32,
> {
    use component_support::{bindings::NeverDecode, presence};
    use maestro_tools::ToolRenderResult;
    use serde_json::json;
    let content_ptr = RefCell::new(None);
    let prior_state = RefCell::new(None);
    let mut tool = component_support::definition::<(), (), NeverDecode, u32>();
    tool.render_call = Some(Rc::new(|_, _, row| {
        *row.state.borrow_mut() += 1;
        Ok(Rc::new(component_support::Line("call custom".into())))
    }));
    tool.render_result = Some(ToolRenderResult::<_, _, maestro_models::Local>::Raw(
        Rc::new(move |result, options, _, row| {
            let ptr = result.content.as_ptr();
            let previous = content_ptr.borrow_mut().replace(ptr);
            let previous_state = prior_state.borrow_mut().replace(Rc::clone(&row.state));
            observed.borrow_mut().push(json!({
                "name":"custom", "expanded":options.expanded,
                "details":presence(result.details),
                "same_details":result.details.map(std::ptr::from_ref)==details,
                "same_content":previous.is_none_or(|previous| previous==ptr),
                "same_state":previous_state.is_none_or(|state| Rc::ptr_eq(&state, &row.state)),
                "args":presence(row.args), "is_error":result.is_error.unwrap(),
                "count":*row.state.borrow(), "content":result.content,
            }));
            Ok(Rc::new(component_support::Line(
                if options.expanded {
                    "expanded"
                } else {
                    "collapsed"
                }
                .into(),
            )))
        }),
    ));
    tool
}

fn reentrant_renderer(
    inner: serde_json::Value,
) -> (Rc<dyn maestro_export::ToolHtmlRenderer>, ExportEvents) {
    use maestro_export::{ToolHtmlRenderer, ToolHtmlRendererDeps, create_tool_html_renderer};
    use maestro_tools::{ToolRenderResult, ToolRendererBinding};
    use serde_json::json;
    // Static callbacks retain a weak renderer whose dependencies must outlive them.
    let theme = maestro_theme::LiveTheme::from_theme(Rc::new(component_support::theme()));
    let events = Rc::new(RefCell::new(Vec::new()));
    let slot = RendererSlot::default();
    let version = Rc::new(std::cell::Cell::new(false));
    let binding = reentrant_binding(&events, &slot, &version, inner);
    let mut replacement = component_support::definition::<(), (), serde_json::Value, bool>();
    replacement.render_result = Some(ToolRenderResult::<_, _, maestro_models::Local>::Raw(
        Rc::new(|_, _, _, _| Ok(Rc::new(component_support::Line("replacement".into())))),
    ));
    let replacement = ToolRendererBinding::from_raw(&replacement).unwrap();
    let lookups = events.clone();
    let renderer: Rc<dyn ToolHtmlRenderer> = Rc::new(create_tool_html_renderer(
        ToolHtmlRendererDeps {
            get_tool_definition: move |name: &str| {
                lookups.borrow_mut().push(json!({"lookup":name,"version":if version.get() { "replacement" } else { "first" }}));
                Ok(Some(if name == "outer" && version.get() {
                    replacement.clone()
                } else {
                    binding.clone()
                }))
            },
            theme,
            cwd: "/work",
            width: None,
            environment: component_support::environment(),
        },
    ));
    *slot.borrow_mut() = Some(Rc::downgrade(&renderer));
    (renderer, events)
}

type ExportEvents = Rc<RefCell<Vec<serde_json::Value>>>;
type RendererSlot = Rc<RefCell<Option<std::rc::Weak<dyn maestro_export::ToolHtmlRenderer>>>>;
fn reentrant_binding(
    events: &ExportEvents,
    slot: &RendererSlot,
    version: &Rc<std::cell::Cell<bool>>,
    inner: serde_json::Value,
) -> maestro_tools::ToolRendererBinding {
    use maestro_export::ToolHtmlResultInput;
    use maestro_tools::{ToolRenderResult, ToolRendererBinding};
    use serde_json::json;
    let mut tool = component_support::definition::<(), (), Details, u32>();
    let seen = events.clone();
    tool.render_call = Some(Rc::new(move |args, _, _| {
        seen.borrow_mut().push(json!({"slot":"call","args":args}));
        Ok(Rc::new(component_support::Line(format!(
            "call {}",
            args.unwrap()["version"].as_str().unwrap()
        ))))
    }));
    let seen = events.clone();
    let nested = Rc::new(std::cell::Cell::new(false));
    let current = version.clone();
    let handle = slot.clone();

    tool.render_result = Some(ToolRenderResult::<_, _, maestro_models::Local>::Typed(
        Rc::new(move |result, options, _, row| {
            let label = &result.details.unwrap().label;
            let last = row
                .last_component
                .as_ref()
                .map(|component| component.render(80).join("\n"));
            seen.borrow_mut().push(json!({"slot":"result","expanded":options.expanded,"details":{"label":label},"args":row.args,"last":last}));
            if !options.expanded && !nested.replace(true) {
                current.set(true);
                let renderer = handle.borrow().as_ref().unwrap().upgrade().unwrap();
                renderer
                    .render_call("id", "different", Some(&json!({"version":"replacement"})))
                    .unwrap();
                renderer
                    .render_result(
                        "id",
                        "inner",
                        ToolHtmlResultInput {
                            content: None,
                            details: Some(&inner),
                            is_error: false,
                        },
                    )
                    .unwrap()
                    .unwrap();
            }
            Ok(Rc::new(component_support::Line(format!(
                "{label}:{}",
                options.expanded
            ))))
        }),
    ));
    ToolRendererBinding::from_definition(&tool)
}

fn edit_result_wrapper(
    callback: maestro_tools::ToolRenderResultCallback<
        serde_json::Value,
        maestro_tools::EditRenderState,
        maestro_models::Threaded,
    >,
    states: &Rc<RefCell<Vec<Rc<RefCell<maestro_tools::EditRenderState>>>>>,
) -> maestro_tools::ToolRendererBinding {
    use maestro_tools::{ToolRenderResult, ToolRendererBinding};
    let seen = states.clone();
    let mut custom =
        component_support::definition::<String, bool, serde_json::Value, Vec<String>>();
    custom.render_result = Some(ToolRenderResult::<_, _, maestro_models::Local>::Raw(
        Rc::new(move |result, options, theme, row| {
            row.state.borrow_mut().push("result".into());
            let state = row
                .renderer_state()
                .unwrap()
                .state::<maestro_tools::EditRenderState>();
            seen.borrow_mut().push(state.clone());
            callback(result, options, theme, &row.with_state(state))
        }),
    ));
    ToolRendererBinding::from_raw(&custom).unwrap()
}

fn check_edit_replacement(
    renderer: &dyn maestro_export::ToolHtmlRenderer,
    result_only: &std::cell::Cell<bool>,
    case: &component_support::bindings::Case<component_support::bindings::Mixed>,
    states: &Rc<RefCell<Vec<Rc<RefCell<maestro_tools::EditRenderState>>>>>,
) {
    use component_support::result_value;
    use maestro_export::ToolHtmlResultInput;
    use serde_json::json;
    let call = renderer
        .render_call("first", "edit", case.input.args.as_ref())
        .unwrap()
        .unwrap();
    result_only.set(true);
    let result = renderer
        .render_result(
            "first",
            "edit",
            ToolHtmlResultInput {
                content: None,
                details: case.input.details.as_ref(),
                is_error: false,
            },
        )
        .unwrap()
        .unwrap();
    result_only.set(false);
    let settled = renderer
        .render_call("first", "edit", case.input.args.as_ref())
        .unwrap()
        .unwrap();
    let other = renderer
        .render_call("second", "edit", case.input.args.as_ref())
        .unwrap()
        .unwrap();
    let states = states.borrow();
    assert_eq!(states.len(), 2);
    assert_eq!(
        json!({"call":call,"result":result_value(result),"settled":settled,"other":other,"same_state":Rc::ptr_eq(&states[0], &states[1])}),
        case.expected
    );
}
/// A tool whose call and result renderers never read the theme they are given.
fn theme_free_definition()
-> maestro_tools::types::local::ToolDefinition<(), (), serde_json::Value, u32> {
    use maestro_tools::ToolRenderResult;
    let mut tool = component_support::definition::<(), (), serde_json::Value, u32>();
    tool.render_call = Some(Rc::new(|_, _, _| {
        Ok(Rc::new(component_support::Line("plain call".into())) as ComponentHandle)
    }));
    tool.render_result = Some(ToolRenderResult::<
        serde_json::Value,
        u32,
        maestro_models::Local,
    >::Raw(Rc::new(|_, _, _, _| {
        Ok(Rc::new(component_support::Line("plain result".into())) as ComponentHandle)
    })));
    tool
}

/// A tool whose call and result renderers colour their row from the theme.
fn theme_reading_definition()
-> maestro_tools::types::local::ToolDefinition<(), (), serde_json::Value, u32> {
    use maestro_tools::ToolRenderResult;
    let styled = |theme: &maestro_theme::LiveTheme| {
        let theme = theme.get().map_err(|error| diagnostic(&error))?;
        let text = theme
            .fg(&maestro_theme::ThemeColor::ToolOutput, "styled")
            .map_err(|error| diagnostic(&error))?;
        Ok(Rc::new(component_support::Line(text)) as ComponentHandle)
    };
    let mut tool = component_support::definition::<(), (), serde_json::Value, u32>();
    tool.render_call = Some(Rc::new(move |_, theme, _| styled(theme)));
    tool.render_result = Some(ToolRenderResult::<
        serde_json::Value,
        u32,
        maestro_models::Local,
    >::Raw(Rc::new(move |_, _, theme, _| styled(theme))));
    tool
}

/// The diagnostic a theme failure becomes at a renderer boundary.
fn diagnostic(error: &maestro_theme::ThemeError) -> maestro_models::DiagnosticErrorInfo {
    maestro_models::DiagnosticErrorInfo::error(error.to_string())
}

/// A theme whose `toolOutput` colour is palette `index`.
fn themed_palette(index: u8) -> maestro_theme::Theme {
    maestro_theme::Theme::new(
        [(
            maestro_theme::ThemeColor::ToolOutput,
            maestro_theme::ColorValue::Index(index),
        )],
        [],
        maestro_theme::ColorMode::Color256,
        maestro_theme::ThemeOptions::default(),
    )
    .unwrap()
}

/// Render `tool` through an HTML renderer over `theme`.
fn themed_renderer(
    tool: &maestro_tools::types::local::ToolDefinition<(), (), serde_json::Value, u32>,
    theme: maestro_theme::LiveTheme,
) -> impl maestro_export::ToolHtmlRenderer {
    use maestro_export::{ToolHtmlRendererDeps, create_tool_html_renderer};
    let binding = maestro_tools::ToolRendererBinding::from_definition(tool);
    create_tool_html_renderer(ToolHtmlRendererDeps {
        get_tool_definition: move |_: &str| Ok(Some(binding.clone())),
        theme,
        cwd: "/work",
        width: None,
        environment: component_support::environment(),
    })
}

#[test]
fn theme_free_renderers_render_when_they_ignore_the_theme() {
    use maestro_export::{ToolHtmlRenderer, ToolHtmlResultInput};
    use serde_json::json;
    let tool = theme_free_definition();
    let renderer = themed_renderer(&tool, maestro_theme::LiveTheme::default());
    let call = renderer
        .render_call("id", "custom", Some(&json!({})))
        .unwrap()
        .unwrap();
    assert_eq!(call, "<div class=\"ansi-line\">plain call</div>");
    let result = renderer
        .render_result(
            "id",
            "custom",
            ToolHtmlResultInput {
                content: None,
                details: Some(&json!({})),
                is_error: false,
            },
        )
        .unwrap()
        .unwrap();
    assert_eq!(result.collapsed, None);
    assert_eq!(
        result.expanded.unwrap(),
        "<div class=\"ansi-line\">plain result</div>"
    );
}

#[test]
fn theme_readers_fall_back_without_a_published_theme() {
    use maestro_export::{ToolHtmlRenderer, ToolHtmlResultInput};
    use serde_json::json;
    let tool = theme_reading_definition();
    let renderer = themed_renderer(&tool, maestro_theme::LiveTheme::default());
    assert_eq!(
        renderer
            .render_call("id", "custom", Some(&json!({})))
            .unwrap(),
        None
    );
    assert!(
        renderer
            .render_result(
                "id",
                "custom",
                ToolHtmlResultInput {
                    content: None,
                    details: Some(&json!({})),
                    is_error: false,
                },
            )
            .unwrap()
            .is_none()
    );
}

/// The six native tools the theme-free rendering cases bind, each with its call and result
/// callbacks; a tool that is not bound here fails the lookup and renders nothing.
fn native_bindings() -> Vec<(&'static str, maestro_tools::ToolRendererBinding)> {
    use maestro_tools::ToolRendererBinding;
    let read = maestro_tools::create_read_tool_definition::<()>(
        "/work".into(),
        maestro_tools::ReadToolOptions::default(),
    );
    let edit = maestro_tools::create_edit_tool_definition::<()>(
        "/work".into(),
        maestro_tools::EditToolOptions::default(),
    );
    let write = maestro_tools::create_write_tool_definition::<()>(
        "/work".into(),
        maestro_tools::WriteToolOptions::default(),
    );
    let find = maestro_tools::create_find_tool_definition::<()>(
        "/work".into(),
        maestro_tools::FindToolOptions::default(),
        std::sync::Arc::new(maestro_tools::ToolsManager::new(
            "/unused/search".into(),
            None,
        )),
    );
    let grep = maestro_tools::create_grep_tool_definition::<()>(
        "/work".into(),
        maestro_tools::GrepToolOptions::default(),
        std::sync::Arc::new(maestro_tools::ToolsManager::new(
            "/unused/search".into(),
            None,
        )),
    );
    let ls = maestro_tools::create_ls_tool_definition::<()>(
        "/work".into(),
        maestro_tools::LsToolOptions::default(),
    );
    vec![
        ("read", ToolRendererBinding::from_definition(&read)),
        ("edit", ToolRendererBinding::from_raw(&edit).unwrap()),
        ("write", ToolRendererBinding::from_raw(&write).unwrap()),
        ("find", ToolRendererBinding::from_raw(&find).unwrap()),
        ("grep", ToolRendererBinding::from_raw(&grep).unwrap()),
        ("ls", ToolRendererBinding::from_raw(&ls).unwrap()),
    ]
}

/// An HTML renderer over those six native tools with no published theme, highlighting
/// through `highlighter`.
fn native_renderer_without_a_theme(
    highlighter: Rc<dyn maestro_theme::SyntaxHighlighter>,
) -> impl maestro_export::ToolHtmlRenderer {
    use maestro_export::{ToolHtmlRendererDeps, create_tool_html_renderer};
    let bindings = native_bindings();
    create_tool_html_renderer(ToolHtmlRendererDeps {
        get_tool_definition: move |name: &str| {
            Ok(bindings
                .iter()
                .find(|(tool, _)| *tool == name)
                .map(|(_, binding)| binding.clone()))
        },
        theme: maestro_theme::LiveTheme::default(),
        cwd: "/work",
        width: None,
        environment: component_support::environment_with(highlighter),
    })
}

/// The same six tools over a highlighter that claims no language.
fn unpublished_native_renderer() -> impl maestro_export::ToolHtmlRenderer {
    native_renderer_without_a_theme(Rc::new(component_support::NoLanguages))
}

#[test]
fn native_calls_fall_back_without_a_published_theme() {
    use maestro_export::ToolHtmlRenderer;
    use serde_json::json;
    let renderer = unpublished_native_renderer();
    for name in ["read", "edit", "write", "find", "grep", "ls"] {
        assert_eq!(
            renderer
                .render_call("id", name, Some(&json!({"path": "notes.txt"})))
                .unwrap(),
            None,
            "{name} call"
        );
    }
}

/// Results without content draw nothing, so a read (visible and hidden), an edit, a write, a
/// find, a grep and a listing render their empty component instead of falling back.
#[test]
fn native_theme_free_result_branches_render_without_a_published_theme() {
    use maestro_export::{ToolHtmlRenderer, ToolHtmlResultInput};
    use serde_json::json;
    let renderer = unpublished_native_renderer();
    let rows = [
        ("read", json!({"path": "notes.txt"})),
        ("read-hidden", json!({"path": "AGENTS.md"})),
        (
            "edit",
            json!({"path": "notes.txt", "oldText": "a", "newText": "b"}),
        ),
        ("write", json!({"path": "notes.txt"})),
        ("find", json!({"pattern": "*.rs"})),
        ("grep", json!({"pattern": "needle"})),
        ("ls", json!({"path": "."})),
    ];
    for (id, args) in rows {
        let name = id.split('-').next().unwrap_or(id);
        assert_eq!(
            renderer.render_call(id, name, Some(&args)).unwrap(),
            None,
            "{id} call"
        );
        let result = renderer
            .render_result(
                id,
                name,
                ToolHtmlResultInput {
                    content: None,
                    details: None,
                    is_error: false,
                },
            )
            .unwrap()
            .unwrap_or_else(|| panic!("{id} result"));
        assert_eq!(result.collapsed, None, "{id} collapsed");
        assert_eq!(result.expanded.as_deref(), Some(""), "{id} expanded");
    }
}

/// A read of a path whose language the syntax registry does not admit colors its rows through
/// the theme, so it falls back; a supported language keeps its custom rendering.
#[test]
fn native_reads_fall_back_for_languages_the_registry_does_not_admit() {
    use maestro_export::{ToolHtmlRenderer, ToolHtmlResultInput};
    use maestro_theme::SyntaxHighlighter as _;
    use serde_json::json;
    let highlighter = Rc::new(maestro_theme::SyntectHighlighter::new().unwrap());
    let renderer = native_renderer_without_a_theme(Rc::clone(&highlighter) as _);
    let read = |path: &str| {
        assert_eq!(
            renderer
                .render_call("id", "read", Some(&json!({"path": path})))
                .unwrap(),
            None,
            "{path} call falls back"
        );
        renderer
            .render_result(
                "id",
                "read",
                ToolHtmlResultInput {
                    content: Some(&json!([{"type": "text", "text": "echo hello"}])),
                    details: None,
                    is_error: false,
                },
            )
            .unwrap()
    };
    for path in [
        "config.fish",
        "styles.sass",
        "schema.graphql",
        "main.tf",
        "main.hcl",
    ] {
        let language = maestro_theme::get_language_from_path(path).unwrap();
        assert!(
            highlighter.supports_language(language),
            "{path} names a language the engine could paint"
        );
        assert!(read(path).is_none(), "{path} result falls back");
    }
    assert!(read("notes.rs").is_some(), "a supported language renders");
}

/// A write error result is drawn in a color, so it falls back instead.
#[test]
fn native_styled_result_branches_fall_back_without_a_published_theme() {
    use maestro_export::{ToolHtmlRenderer, ToolHtmlResultInput};
    use serde_json::json;
    let renderer = unpublished_native_renderer();
    assert_eq!(
        renderer
            .render_call("id", "write", Some(&json!({"path": "notes.txt"})))
            .unwrap(),
        None
    );
    assert!(
        renderer
            .render_result(
                "id",
                "write",
                ToolHtmlResultInput {
                    content: Some(&json!([{"type": "text", "text": "no such file"}])),
                    details: None,
                    is_error: true,
                },
            )
            .unwrap()
            .is_none(),
        "a styled error falls back"
    );
}

#[test]
fn published_theme_reaches_callbacks_and_is_reread_when_republished() {
    use maestro_export::ToolHtmlRenderer;
    use serde_json::json;
    let state = maestro_theme::ThemeState::new(maestro_theme::ThemeDirectories {
        themes_dir: String::new(),
        custom_themes_dir: String::new(),
    });
    let live = state.theme();
    state
        .set_theme_instance(Rc::new(themed_palette(2)))
        .unwrap();
    let tool = theme_reading_definition();
    let renderer = themed_renderer(&tool, live);
    let first = renderer
        .render_call("id", "custom", Some(&json!({})))
        .unwrap()
        .unwrap();
    assert!(first.contains(&maestro_theme::ansi256_to_hex(2)));
    state
        .set_theme_instance(Rc::new(themed_palette(5)))
        .unwrap();
    let second = renderer
        .render_call("id", "custom", Some(&json!({})))
        .unwrap()
        .unwrap();
    assert!(second.contains(&maestro_theme::ansi256_to_hex(5)));
}
