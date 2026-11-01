//! The recorded registration matrix and registered artifact cards in mounted message rows.
use crate::{
    contributions_support::{body, select, text},
    tool_cards_support::{Expected, NativeDetail, Registration},
};
use maestro_app::{JsonObject, ToolCall, ToolResultMessage};
use maestro_web::{
    ArtifactsToolRenderer, AssistantMessage, ToolMessage,
    components::messages::ToolResults,
    dom::{DomContent, DomView},
    get_tool_renderer, register_tool_renderer, render_tool, set_show_json_mode,
    tools::{
        renderer_registry::TOOL_RENDERERS,
        types::{ToolParams, ToolRenderResult, ToolRenderer, ToolRendererBinding},
    },
};
use serde_json::{Value, json};
use std::{
    cell::Cell,
    collections::HashMap,
    rc::Rc,
    sync::{Arc, RwLock},
};
use wasm_bindgen::JsCast as _;
use web_sys::{Element, HtmlElement};

/// Result details that cannot be serialized; calling one counts a read.
type Details = Rc<dyn Fn()>;

/// Mount prepared content in the page, compare it with its recorded card and remove it.
fn observe(view: &DomView, shown: &ToolRenderResult, expected: &Expected) {
    assert_eq!(shown.is_custom, expected.is_custom);
    let root = shown.content.mount(view).unwrap();
    body().append_child(&root.node).unwrap();
    let element: &Element = root.node.unchecked_ref();
    super::observe_card(element, expected, 0);
    element.remove();
}

/// The built-in command card prepared from a registered object argument.
fn registered(view: &DomView, expected: &Expected) {
    let params = json!({"command": "echo registered"});
    let object = ToolParams::object(params.as_object().unwrap());
    let shown = render_tool::<JsonObject, Details>("bash", Some(object), None, false).unwrap();
    observe(view, &shown, expected);
}

/// Mount `row` once and return its root element, already removed from the page.
fn row_element<D: 'static>(view: &DomView, row: &ToolMessage<D>) -> Element {
    let root = row.mount(view).unwrap();
    body().append_child(&root.node).unwrap();
    let element: Element = root.node.unchecked_into();
    element.remove();
    element
}

/// The initial names and card, the four whole-value routes, native details through a tool row,
/// replacement, forced JSON, removal and the retained original handle replay their record.
pub(super) fn recorded(view: &Rc<DomView>) {
    let recorded = crate::tool_cards_support::fixture();
    let names = TOOL_RENDERERS.with(|all| all.borrow().keys().cloned().collect::<Vec<_>>());
    assert_eq!(names, recorded.registration.initial_names);
    let original = get_tool_renderer::<JsonObject, Details>("bash").unwrap();
    let again = get_tool_renderer::<JsonObject, Details>("bash").unwrap();
    assert!(Rc::ptr_eq(&original, &again));
    registered(view, &recorded.registration.initial);
    assert_eq!(recorded.value_routes.len(), 4);
    for route in recorded.value_routes {
        let params = Some(ToolParams::json(&route.input.params));
        let result = Some(&route.input.result);
        let shown = render_tool("bash", params, result, route.input.is_streaming).unwrap();
        observe(view, &shown, &route.expected);
    }
    let row = native_row(view, recorded.native_detail);
    replacement(view, &row, &recorded.registration);
    let params = json!({"command": "echo retained"});
    let shown = original.render(params.as_object(), None, false).unwrap();
    observe(view, &shown, &recorded.registration.retained_original);
}

/// A command row whose result details count each read.
struct NativeRow {
    /// The mounted row's operands.
    row: ToolMessage<Details>,
    /// Detail reads so far.
    reads: Rc<Cell<usize>>,
}

/// Build the recorded native-details row and check its card, unread details and dispatch.
fn native_row(view: &DomView, recorded: NativeDetail) -> NativeRow {
    let input = recorded.input;
    let reads = Rc::new(Cell::new(0));
    let observed = Rc::clone(&reads);
    let result = input.result.unwrap();
    let result = ToolResultMessage {
        tool_call_id: result.tool_call_id,
        tool_name: result.tool_name,
        content: result.content,
        timestamp: result.timestamp,
        is_error: result.is_error,
        details: Some(Rc::new(move || observed.set(observed.get() + 1)) as Details),
    };
    let row = ToolMessage {
        tool_call: Some(Rc::new(ToolCall {
            id: "call".into(),
            name: "bash".into(),
            arguments: Value::Object(input.params.unwrap()),
            thought_signature: None,
        })),
        result: Some(Rc::new(result)),
        is_streaming: input.is_streaming,
        ..ToolMessage::default()
    };
    let console = select(&row_element(view, &row), ".maestro-console pre");
    assert_eq!(text(&console), recorded.expected.console[0].content);
    assert_eq!(reads.get(), recorded.detail_reads);
    let value = json!({"command": "echo native"});
    let shown = render_tool(
        "bash",
        Some(ToolParams::json(&value)),
        row.result.as_deref(),
        false,
    )
    .unwrap();
    observe(view, &shown, &recorded.expected);
    NativeRow { row, reads }
}

/// Replacement, forced JSON, restoration and removal through the same native row.
fn replacement(view: &DomView, native: &NativeRow, recorded: &Registration) {
    let replacement: Rc<dyn ToolRenderer<JsonObject, Details>> =
        Rc::new(crate::recipes::Renderer(Rc::new(|_, _, _| {
            Ok(crate::recipes::content("replacement", true))
        })));
    register_tool_renderer(
        "bash",
        ToolRendererBinding::from_raw(Rc::clone(&replacement)),
    );
    let current = || get_tool_renderer::<JsonObject, Details>("bash");
    assert_eq!(
        Rc::ptr_eq(&replacement, &current().unwrap()),
        recorded.replaced
    );
    assert_eq!(text(&row_element(view, &native.row)), "replacement");
    set_show_json_mode(true);
    json_listing(&row_element(view, &native.row));
    registered(view, &recorded.forced);
    set_show_json_mode(false);
    assert_eq!(
        Rc::ptr_eq(&replacement, &current().unwrap()),
        recorded.restored
    );
    drop(TOOL_RENDERERS.with(|all| all.borrow_mut().shift_remove("bash")));
    json_listing(&row_element(view, &native.row));
    registered(view, &recorded.removed);
    assert_eq!(current().is_none(), recorded.still_absent);
    assert_eq!(native.reads.get(), 0, "no card reads native details");
}

/// A default JSON card: argument listings, no command console.
fn json_listing(row: &Element) {
    assert!(row.query_selector(".maestro-console").unwrap().is_none());
    assert!(row.query_selector(".maestro-code").unwrap().is_some());
}

/// The artifact call every registered row receives.
fn artifact_call() -> Rc<ToolCall> {
    Rc::new(serde_json::from_value(json!({"type":"toolCall","id":"artifact-call","name":"artifacts","arguments":{"command":"create","filename":"literal.rs","content":"literal code"}})).unwrap())
}

/// One registration serving all three argument families, its phases through tool and assistant
/// rows, replacement, forced JSON and removal; rows never read or retain native details.
pub(super) fn artifacts(view: &Rc<DomView>) {
    let renderer = Rc::new(ArtifactsToolRenderer::new(None));
    let binding = ToolRendererBinding::from_raw::<JsonObject, Details>(renderer.clone())
        .with_raw::<Value, Details>(renderer.clone())
        .with_raw::<maestro_app::browser::tools::artifacts::ArtifactsParams, Details>(renderer);
    let names = TOOL_RENDERERS.with(|all| all.borrow().keys().cloned().collect::<Vec<_>>());
    assert!(!names.iter().any(|name| name == "artifacts"));
    register_tool_renderer("artifacts", binding.clone());
    let reads = Rc::new(Cell::new(0));
    let observed = Rc::clone(&reads);
    let details: Details = Rc::new(move || observed.set(observed.get() + 1));
    let weak = Rc::downgrade(&details);
    let mut result = crate::recipes::message(Some(details));
    result.is_error = false;
    result.content = vec![maestro_app::UserBlock::Text(maestro_app::TextContent::new(
        "literal result".into(),
    ))];
    artifact_adapters(view, &result);
    artifact_phases(view, &result);
    artifact_registry(view, &result, binding);
    assert_eq!(reads.get(), 0);
    drop(result);
    assert!(
        weak.upgrade().is_none(),
        "rows retain no native result details"
    );
    drop(TOOL_RENDERERS.with(|all| all.borrow_mut().shift_remove("artifacts")));
    let after = TOOL_RENDERERS.with(|all| all.borrow().keys().cloned().collect::<Vec<_>>());
    assert_eq!(after, names);
}

/// The object, whole-value and typed adapters prepare the same created card; typed arguments
/// are never formatted.
fn artifact_adapters(view: &DomView, result: &ToolResultMessage<Details>) {
    let params = json!({"command":"create","filename":"literal.rs","content":"literal code"});
    let typed: maestro_app::browser::tools::artifacts::ArtifactsParams =
        serde_json::from_value(params.clone()).unwrap();
    let prepared = [
        render_tool(
            "artifacts",
            Some(ToolParams::object(params.as_object().unwrap())),
            Some(result),
            false,
        ),
        render_tool(
            "artifacts",
            Some(ToolParams::json(&params)),
            Some(result),
            false,
        ),
        render_tool(
            "artifacts",
            Some(ToolParams::typed(&typed, |_| {
                panic!("registered native operands must not be formatted")
            })),
            Some(result),
            false,
        ),
    ];
    for shown in prepared {
        let shown = shown.unwrap();
        assert!(!shown.is_custom);
        let root = shown.content.mount(view).unwrap();
        let card: &Element = root.node.unchecked_ref();
        assert_eq!(label(card), "Created artifact");
        assert_eq!(
            text(&select(card, ".maestro-code pre code")),
            "literal code"
        );
    }
}

/// Pending, completed and failed phases in a tool row and an assistant row: caption, pill and
/// the code or error body after expansion.
fn artifact_phases(view: &DomView, result: &ToolResultMessage<Details>) {
    let call = artifact_call();
    for (phase, caption) in [
        (0, "Creating artifact"),
        (1, "Created artifact"),
        (2, "Creating artifact"),
    ] {
        let mut result = result_copy(result);
        result.is_error = phase == 2;
        let result = Rc::new(result);
        let row = ToolMessage {
            tool_call: Some(Rc::clone(&call)),
            result: (phase != 0).then(|| Rc::clone(&result)),
            pending: phase == 0,
            ..ToolMessage::default()
        };
        artifact_row(&row_element(view, &row), caption, phase == 2);
        let message = assistant(&call);
        let results = HashMap::from([(call.id.clone(), Rc::clone(&result))]);
        let assistant = AssistantMessage {
            message: Some(message),
            tool_results_by_id: Some(ToolResults::Native(Rc::new(if phase == 0 {
                HashMap::new()
            } else {
                results
            }))),
            is_streaming: phase == 0,
            ..AssistantMessage::default()
        };
        let root = assistant.mount(view).unwrap();
        body().append_child(&root.node).unwrap();
        let element: Element = root.node.unchecked_into();
        artifact_row(&element, caption, phase == 2);
        element.remove();
    }
}

/// A settled assistant message carrying only `call`.
fn assistant(call: &ToolCall) -> maestro_app::SharedAssistantMessage {
    let message = json!({
        "role":"assistant", "content":[call], "api":"controlled", "provider":"controlled",
        "model":"controlled", "usage":maestro_app::Usage::default(), "stopReason":"stop",
        "timestamp":0,
    });
    Arc::new(RwLock::new(serde_json::from_value(message).unwrap()))
}

/// The same envelope with shared native details.
fn result_copy(result: &ToolResultMessage<Details>) -> ToolResultMessage<Details> {
    ToolResultMessage {
        tool_call_id: result.tool_call_id.clone(),
        tool_name: result.tool_name.clone(),
        content: result.content.clone(),
        details: result.details.clone(),
        is_error: result.is_error,
        timestamp: result.timestamp,
    }
}

/// The caption's own label, without the text of a filename pill beside it.
fn label(card: &Element) -> String {
    let caption = select(card, ".maestro-tool-caption");
    if caption
        .query_selector(".maestro-artifact-pill")
        .unwrap()
        .is_some()
    {
        text(
            &caption
                .first_element_child()
                .unwrap()
                .first_child()
                .unwrap(),
        )
    } else {
        text(&caption)
    }
}

/// Expand one artifact card and compare its caption, pill and body.
fn artifact_row(row: &Element, caption: &str, failed: bool) {
    assert_eq!(label(row), caption);
    select(row, ".maestro-tool-toggle")
        .unchecked_into::<HtmlElement>()
        .click();
    assert_eq!(text(&select(row, ".maestro-artifact-pill")), "literal.rs");
    if failed {
        assert_eq!(
            text(&select(row, "div.maestro-tool-error")),
            "literal result"
        );
    } else {
        assert_eq!(text(&select(row, ".maestro-code pre code")), "literal code");
    }
}

/// Replacement, forced JSON, removal and re-registration through one tool row.
fn artifact_registry(
    view: &DomView,
    result: &ToolResultMessage<Details>,
    original: ToolRendererBinding,
) {
    let replacement: Rc<dyn ToolRenderer<JsonObject, Details>> =
        Rc::new(crate::recipes::Renderer(Rc::new(|_, _, _| {
            Ok(crate::recipes::content("replacement artifact", true))
        })));
    register_tool_renderer(
        "artifacts",
        ToolRendererBinding::from_raw(Rc::clone(&replacement)),
    );
    let current = get_tool_renderer::<JsonObject, Details>("artifacts").unwrap();
    assert!(Rc::ptr_eq(&replacement, &current));
    let row = ToolMessage {
        tool_call: Some(artifact_call()),
        result: Some(Rc::new(result_copy(result))),
        ..ToolMessage::default()
    };
    assert_eq!(text(&row_element(view, &row)), "replacement artifact");
    set_show_json_mode(true);
    let forced = row_element(view, &row);
    assert!(
        forced
            .query_selector(".maestro-artifact-pill")
            .unwrap()
            .is_none()
    );
    json_listing(&forced);
    set_show_json_mode(false);
    assert_eq!(text(&row_element(view, &row)), "replacement artifact");
    drop(TOOL_RENDERERS.with(|all| all.borrow_mut().shift_remove("artifacts")));
    let removed = row_element(view, &row);
    assert!(
        removed
            .query_selector(".maestro-artifact-pill")
            .unwrap()
            .is_none()
    );
    json_listing(&removed);
    register_tool_renderer("artifacts", original);
    let restored = row_element(view, &row);
    assert_eq!(label(&restored), "Created artifact");
}
