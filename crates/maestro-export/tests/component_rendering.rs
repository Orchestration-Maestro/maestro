//! Standard component renderer behavior.
#![cfg(test)]
#[test]
fn result_padding_keeps_red_one_and_plain_two() {
    component_support::check("result_padding_keeps_red_one_and_plain_two");
}
mod component_support;
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
    let tool = Rc::new(tool);
    let renderer = create_tool_html_renderer(ToolHtmlRendererDeps {
        get_tool_definition: |_: &str| Ok(Some(Rc::clone(&tool))),
        theme: &theme,
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

use maestro_tui::{Component, tui::ComponentHandle};
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
struct Line(String);
impl Component for Line {
    fn render(&self, _: usize) -> Vec<String> {
        vec![self.0.clone()]
    }
}

type SavedStates = Rc<RefCell<Vec<Rc<RefCell<State>>>>>;
fn typed_definition(
    states: &SavedStates,
) -> maestro_tools::ToolDefinition<u32, Parameters, Details, State> {
    let saved = Rc::clone(states);
    let mut tool = component_support::definition::<u32, Parameters, Details, State>();
    tool.render_result = Some(maestro_tools::ToolRenderResult::Typed(Rc::new(
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
            Ok(Rc::new(Line(text)) as ComponentHandle)
        },
    )));
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
    let mut tool: maestro_tools::ToolDefinition = component_support::definition();
    let seen = Rc::clone(&reached);
    tool.render_call = Some(Rc::new(move |_, _, _| {
        seen.set(seen.get() + 1);
        Ok(nest(Rc::new(Fallible(failing_phase == 0)), padded))
    }));
    let seen = Rc::clone(&reached);
    tool.render_result = Some(maestro_tools::ToolRenderResult::Typed(Rc::new(
        move |_, options, _, _| {
            seen.set(seen.get() + 1);
            let phase = if options.expanded { 2 } else { 1 };
            Ok(nest(Rc::new(Fallible(failing_phase == phase)), padded))
        },
    )));
    let tool = Rc::new(tool);
    let theme = component_support::theme();
    let renderer = create_tool_html_renderer(ToolHtmlRendererDeps {
        get_tool_definition: |_: &str| Ok(Some(Rc::clone(&tool))),
        theme: &theme,
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
    tool.render_result = Some(maestro_tools::ToolRenderResult::Typed(Rc::new(
        move |result, options, _, _| {
            seen.borrow_mut()
                .push(Rc::clone(&result.details.unwrap().token));
            Ok(Rc::new(Line(format!("phase:{}", options.expanded))))
        },
    )));
    let tool = Rc::new(tool);
    let theme = component_support::theme();
    let renderer = create_tool_html_renderer(ToolHtmlRendererDeps {
        get_tool_definition: |_: &str| Ok(Some(Rc::clone(&tool))),
        theme: &theme,
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
        get_tool_definition: |_: &str| Ok(Some(Rc::clone(&tool))),
        theme: &theme,
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
        get_tool_definition: |_: &str| Ok(Some(Rc::clone(&tool))),
        theme: &theme,
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
) -> maestro_tools::ToolDefinition<(), (), serde_json::Value, u32> {
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
    tool.render_result = Some(ToolRenderResult::Raw(Rc::new(
        move |result, options, theme, row| {
            assert_eq!(*row.state.borrow(), 1);
            let state = row.renderer_state().unwrap().state::<EditRenderState>();
            seen.borrow_mut().push(Rc::clone(&state));
            let fractional_change = result
                .details
                .and_then(|d| d.get("firstChangedLine"))
                .and_then(serde_json::Value::as_f64)
                == Some(1.75);
            let alternate = native_palette(3);
            let theme = if fractional_change { &alternate } else { theme };
            let output = native_result(result, options, theme, &row.with_state(state))?;
            if fractional_change {
                let call = retained.borrow().as_ref().unwrap().clone();
                let html = maestro_export::ansi_lines_to_html(&call.render(80));
                assert!(html.contains("#808000"));
                assert!(html.contains("persisted"));
            }
            Ok(output)
        },
    )));
    tool
}

/// Publish colors consumed by real native edit callbacks.
fn native_environment() -> (
    Rc<maestro_theme::Theme>,
    Rc<maestro_tools::ToolRenderEnvironment>,
) {
    let theme = native_palette(2);
    let state = maestro_theme::ThemeState::new(
        maestro_theme::ThemeDirectories {
            themes_dir: String::new(),
            custom_themes_dir: String::new(),
        },
        Rc::new(maestro_theme::NativeThemeOperations),
    );
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
