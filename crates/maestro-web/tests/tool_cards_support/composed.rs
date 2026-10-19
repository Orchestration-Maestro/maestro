//! Default binding observed through the containing message row.
#![cfg(test)]
use super::{Expected, NativeDetail, Registration, ValueRoute, fixture, observe};
use crate::{
    markdown_support::{Surface, check, js},
    recipes,
    support::reading_view::view,
};
use maestro_app::{JsonObject, ToolCall, ToolResultMessage};
use maestro_web::{
    ToolMessage, ToolMessageProps, ToolRenderer, get_tool_renderer, register_tool_renderer,
    render_tool, set_show_json_mode,
    tools::{
        renderer_registry::TOOL_RENDERERS,
        types::{ToolParams, ToolRenderState, ToolRendererBinding},
    },
};
use std::{cell::Cell, rc::Rc};

type Native = Rc<dyn Fn()>;
struct Case {
    call: ToolCall,
    result: ToolResultMessage<Native>,
    streaming: bool,
    reads: Rc<Cell<usize>>,
    expected: Expected,
    expected_reads: usize,
}
impl Case {
    fn new(recorded: NativeDetail) -> Self {
        let input = recorded.input;
        assert!(input.language.is_none());
        let call = ToolCall {
            id: "call".into(),
            name: "bash".into(),
            arguments: input.params.unwrap(),
            thought_signature: None,
        };
        let result = input.result.unwrap();
        let reads = Rc::new(Cell::new(0));
        let observed = reads.clone();
        let result = ToolResultMessage {
            tool_call_id: result.tool_call_id,
            tool_name: result.tool_name,
            content: result.content,
            timestamp: result.timestamp,
            is_error: result.is_error,
            details: Some(Rc::new(move || observed.set(observed.get() + 1)) as Native),
        };
        Self {
            call,
            result,
            streaming: input.is_streaming,
            reads,
            expected: recorded.expected,
            expected_reads: recorded.detail_reads,
        }
    }
    fn draw(&self, surface: &mut Surface, row: &mut ToolMessage) {
        let host = surface.host();
        surface.draw(|ui, frame| {
            row.render(
                ui,
                ToolMessageProps {
                    tool_call: &self.call,
                    tool: None,
                    result: Some(&self.result),
                    pending: false,
                    aborted: false,
                    is_streaming: self.streaming,
                },
                view(&host, frame),
            )
            .unwrap()
        });
    }
}
pub fn composed() {
    let recorded = fixture();
    let mut surface = Surface::new();
    let names = TOOL_RENDERERS.with(|all| all.borrow().keys().cloned().collect::<Vec<_>>());
    assert_eq!(names, recorded.registration.initial_names);
    let original = get_tool_renderer::<JsonObject, Native>("bash").unwrap();
    assert!(Rc::ptr_eq(
        &original,
        &get_tool_renderer::<JsonObject, Native>("bash").unwrap()
    ));
    registered(&mut surface, &recorded.registration.initial);
    let case = Case::new(recorded.native_detail);
    native(&mut surface, &case, recorded.value_routes);
    registration(&mut surface, &case, &recorded.registration);
    let params = serde_json::from_value(serde_json::json!({"command":"echo retained"})).unwrap();
    let recipe = original.render(Some(&params), None, false).unwrap();
    let mut state = ToolRenderState::default();
    observe(
        &mut surface,
        &recipe,
        &mut state,
        &recorded.registration.retained_original,
    );
    drop(state);
    check(
        "document.querySelectorAll('.maestro-console,.maestro-code,.maestro-tool-caption').length===0",
    );
}
fn native(surface: &mut Surface, case: &Case, routes: Vec<ValueRoute>) {
    let mut row = ToolMessage::new();
    case.draw(surface, &mut row);
    assert_eq!(
        js("document.querySelector('.maestro-console pre').textContent")
            .as_string()
            .unwrap(),
        case.expected.console[0].content
    );
    assert_eq!(case.reads.get(), case.expected_reads);
    drop(row);
    for route in routes {
        let recipe = render_tool(
            "bash",
            Some(ToolParams::json(&route.input.params)),
            Some(&route.input.result),
            route.input.is_streaming,
        )
        .unwrap();
        let mut state = ToolRenderState::default();
        observe(surface, &recipe, &mut state, &route.expected);
    }
    let value = serde_json::json!({"command":"echo native"});
    let recipe = render_tool(
        "bash",
        Some(ToolParams::json(&value)),
        Some(&case.result),
        false,
    )
    .unwrap();
    let mut state = ToolRenderState::default();
    observe(surface, &recipe, &mut state, &case.expected);
}
fn registration(surface: &mut Surface, case: &Case, recorded: &Registration) {
    let replacement: Rc<dyn ToolRenderer<JsonObject, Native>> =
        Rc::new(recipes::Renderer(Rc::new(|_, _, _| {
            Ok(recipes::content("replacement", true))
        })));
    register_tool_renderer("bash", ToolRendererBinding::from_raw(replacement.clone()));
    assert_eq!(
        Rc::ptr_eq(
            &replacement,
            &get_tool_renderer::<JsonObject, Native>("bash").unwrap()
        ),
        recorded.replaced
    );
    let mut row = ToolMessage::new();
    case.draw(surface, &mut row);
    assert!(
        surface
            .painted
            .iter()
            .any(|(text, _)| text == "replacement")
    );
    set_show_json_mode(true);
    case.draw(surface, &mut row);
    check(
        "document.querySelector('.maestro-console')===null && document.querySelector('.maestro-code')!==null",
    );
    drop(row);
    registered(surface, &recorded.forced);
    set_show_json_mode(false);
    assert_eq!(
        Rc::ptr_eq(
            &replacement,
            &get_tool_renderer::<JsonObject, Native>("bash").unwrap()
        ),
        recorded.restored
    );
    drop(TOOL_RENDERERS.with(|all| all.borrow_mut().shift_remove("bash")));
    let mut row = ToolMessage::new();
    case.draw(surface, &mut row);
    check(
        "document.querySelector('.maestro-console')===null && document.querySelector('.maestro-code')!==null",
    );
    drop(row);
    registered(surface, &recorded.removed);
    assert_eq!(
        get_tool_renderer::<JsonObject, Native>("bash").is_none(),
        recorded.still_absent
    );
}
fn registered(surface: &mut Surface, expected: &Expected) {
    let params = serde_json::from_value(serde_json::json!({"command":"echo registered"})).unwrap();
    let recipe =
        render_tool::<_, Native>("bash", Some(ToolParams::object(&params)), None, false).unwrap();
    let mut state = ToolRenderState::default();
    observe(surface, &recipe, &mut state, expected);
}
