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
        create_state: State::default,
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
    tool.render_result = Some(Rc::new(move |result, options, _, ctx| {
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
    }));
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
    tool.render_result = Some(Rc::new(move |_, options, _, _| {
        seen.set(seen.get() + 1);
        let phase = if options.expanded { 2 } else { 1 };
        Ok(nest(Rc::new(Fallible(failing_phase == phase)), padded))
    }));
    let tool = Rc::new(tool);
    let theme = component_support::theme();
    let renderer = create_tool_html_renderer(ToolHtmlRendererDeps {
        get_tool_definition: |_: &str| Ok(Some(Rc::clone(&tool))),
        theme: &theme,
        cwd: "/work",
        width: None,
        environment: component_support::environment(),
        create_state: || serde_json::json!({}),
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
