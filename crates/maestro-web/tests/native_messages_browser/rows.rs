//! Tool, debug and abort rows mounted directly in the page with original operands.
use crate::contributions_support::{body, call, computed, localized_view, rgb, select, text};
use maestro_app::{JsonObject, ToolCall, ToolResultMessage};
use maestro_web::{
    components::messages::{AbortedMessage, ToolMessage, ToolMessageDebugView},
    dom::{DomContent, DomRoot, DomView},
    tools::types::{
        ToolRenderResult, ToolRenderer, ToolRendererBinding, ToolResultMessageView, ToolUiError,
    },
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::JsCast as _;
use web_sys::Element;

/// Native details that cannot be serialized, observed by identity.
pub(super) type Details = Rc<Cell<u32>>;
/// The recorded transcript fixture.
pub(super) fn fixture() -> Value {
    serde_json::from_str(include_str!("../fixtures/native_transcript/messages.json")).unwrap()
}
/// Result call ID, tool name, error flag, content, timestamp and details as a callback saw them.
type Envelope = (
    String,
    String,
    bool,
    Vec<maestro_app::UserBlock>,
    f64,
    Option<Details>,
);
/// One renderer invocation as the callback observed it.
pub(super) struct Seen {
    /// Address of the borrowed argument object.
    params: usize,
    /// Selected streaming flag.
    streaming: bool,
    /// Result envelope fields and original details, when a result was supplied.
    pub(super) result: Option<Envelope>,
}
/// Content each renderer call returns.
#[derive(Clone, Copy)]
pub(super) enum Output {
    /// Content released into a counter when its root drops.
    Tracked,
    /// Content whose mount fails.
    FailingMount,
    /// A renderer failure before any content exists.
    Refused,
}
/// A registered page-content renderer recording each call.
pub(super) struct Observer {
    /// Every call, oldest first.
    pub(super) seen: RefCell<Vec<Seen>>,
    /// Whether content omits the ordinary card.
    pub(super) custom: Cell<bool>,
    /// The next calls' content.
    pub(super) output: Cell<Output>,
    /// Text the tracked content shows.
    pub(super) text: RefCell<&'static str>,
    /// Roots of tracked content released so far.
    pub(super) released: Rc<Cell<u32>>,
}
impl Observer {
    /// Register a fresh observer as `name`'s page-content renderer.
    pub(super) fn register(name: &str) -> Rc<Self> {
        let observer = Rc::new(Self {
            seen: RefCell::default(),
            custom: Cell::new(true),
            output: Cell::new(Output::Tracked),
            text: RefCell::new("native renderer"),
            released: Rc::default(),
        });
        maestro_web::register_tool_renderer(name, ToolRendererBinding::from_dom(observer.clone()));
        observer
    }
}
impl ToolRenderer<JsonObject, Details, Box<dyn DomContent>> for Observer {
    fn render(
        &self,
        params: Option<&JsonObject>,
        result: Option<ToolResultMessageView<'_, Details>>,
        streaming: bool,
    ) -> Result<ToolRenderResult<Box<dyn DomContent>>, ToolUiError> {
        let content: Box<dyn DomContent> = match self.output.get() {
            Output::Refused => return Err(ToolUiError::Presentation("refused".into())),
            Output::FailingMount => Box::new(FailingMount),
            Output::Tracked => Box::new(Tracked(*self.text.borrow(), self.released.clone())),
        };
        self.seen.borrow_mut().push(Seen {
            params: params.map_or(0, |params| std::ptr::from_ref(params) as usize),
            streaming,
            result: result.map(|result| {
                (
                    result.tool_call_id().into(),
                    result.tool_name().into(),
                    result.is_error(),
                    result.content().to_vec(),
                    result.timestamp(),
                    result.details().cloned(),
                )
            }),
        });
        Ok(ToolRenderResult {
            content,
            is_custom: self.custom.get(),
        })
    }
}
/// Paragraph text whose root owner counts its release.
struct Tracked(&'static str, Rc<Cell<u32>>);
/// Counts one release when dropped.
struct Release(Rc<Cell<u32>>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
impl DomContent for Tracked {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let root = view.document.create_element("p").unwrap();
        root.set_text_content(Some(self.0));
        Ok(DomRoot {
            node: root.into(),
            owner: Some(Box::new(Release(self.1.clone()))),
        })
    }
    fn update(&self, root: &DomRoot, _: &DomView) -> Result<(), ToolUiError> {
        root.node.set_text_content(Some(self.0));
        Ok(())
    }
}
/// Content refusing to mount.
struct FailingMount;
impl DomContent for FailingMount {
    fn mount(&self, _: &DomView) -> Result<DomRoot, ToolUiError> {
        Err(ToolUiError::Presentation("mount refused".into()))
    }
    fn update(&self, _: &DomRoot, _: &DomView) -> Result<(), ToolUiError> {
        Err(ToolUiError::Presentation("update refused".into()))
    }
}
/// The recorded call as an application record.
pub(super) fn tool_call(fixture: &Value) -> Rc<ToolCall> {
    Rc::new(serde_json::from_value(fixture["call"].clone()).unwrap())
}
/// The recorded result envelope carrying native `details`.
pub(super) fn tool_result(fixture: &Value, details: &Details) -> Rc<ToolResultMessage<Details>> {
    let recorded = &fixture["toolResult"];
    Rc::new(ToolResultMessage {
        tool_call_id: recorded["toolCallId"].as_str().unwrap().into(),
        tool_name: recorded["toolName"].as_str().unwrap().into(),
        content: serde_json::from_value(recorded["content"].clone()).unwrap(),
        details: Some(details.clone()),
        is_error: recorded["isError"].as_bool().unwrap(),
        timestamp: recorded["timestamp"].as_f64().unwrap(),
        usage: None,
    })
}
/// The row's ordinary card, if shown.
pub(super) fn card(root: &DomRoot) -> Option<Element> {
    root.node
        .unchecked_ref::<Element>()
        .query_selector(":scope > .maestro-tool-message-card")
        .unwrap()
}

/// Each recorded pending, streaming, result and abort combination reaches the selected renderer with
/// the original arguments and envelope; an abort replaces the result and ends streaming.
pub(super) fn recorded_combinations(view: &Rc<DomView>, fixture: &Value) {
    let observer = Observer::register("registered");
    let call = tool_call(fixture);
    let details = Rc::new(Cell::new(43));
    let original = tool_result(fixture, &details);
    let arguments = std::ptr::from_ref(call.arguments.as_object().unwrap()) as usize;
    let cases = fixture["tool"].as_array().unwrap();
    assert_eq!(cases.len(), 7);
    for (index, case) in cases.iter().enumerate() {
        let flag = |name: &str| case["input"][name].as_bool().unwrap();
        let row = ToolMessage::<Details> {
            tool_call: Some(call.clone()),
            tool: None,
            result: flag("hasResult").then(|| original.clone()),
            pending: flag("pending"),
            aborted: flag("aborted"),
            is_streaming: flag("streaming"),
        };
        let before = js_sys::Date::now();
        let root = row.mount(view).unwrap();
        let after = js_sys::Date::now();
        body().append_child(&root.node).unwrap();
        let seen = observer.seen.borrow_mut().pop().unwrap();
        let expected = &case["expected"];
        assert!(expected["sameParams"].as_bool().unwrap());
        assert_eq!(seen.params, arguments, "{index}");
        assert_eq!(
            seen.streaming,
            expected["streaming"].as_bool().unwrap(),
            "{index}"
        );
        envelope(
            seen.result.as_ref(),
            &expected["result"],
            (&original, &details),
            [before, after],
        );
        assert_eq!(text(&root.node), "native renderer", "{index}");
        assert!(card(&root).is_none(), "{index}");
        root.node.unchecked_ref::<Element>().remove();
    }
    maestro_web::tools::renderer_registry::TOOL_RENDERERS
        .with(|all| all.borrow_mut().shift_remove("registered"));
}
/// Compare one observed envelope with its recorded fields.
fn envelope(
    seen: Option<&Envelope>,
    expected: &Value,
    (original, details): (&ToolResultMessage<Details>, &Details),
    [before, after]: [f64; 2],
) {
    if expected.get("undefined").is_some() {
        assert!(seen.is_none());
        return;
    }
    let (id, name, error, content, timestamp, shown) = seen.unwrap();
    assert_eq!(id, expected["toolCallId"].as_str().unwrap());
    assert_eq!(name, expected["toolName"].as_str().unwrap());
    assert_eq!(*error, expected["isError"].as_bool().unwrap());
    assert_eq!(
        content,
        &serde_json::from_value::<Vec<_>>(expected["content"].clone()).unwrap()
    );
    if expected["timestamp"] == "CURRENT_TIME" {
        assert!(*timestamp >= before && *timestamp <= after && timestamp.fract() == 0.);
        assert!(shown.is_none());
    } else {
        assert_eq!(
            timestamp.to_bits(),
            expected["timestamp"].as_f64().unwrap().to_bits()
        );
        assert!(Rc::ptr_eq(shown.as_ref().unwrap(), details));
        assert_eq!(original.content, *content);
    }
}
/// A selected tool name wins over the call's name unless it is empty; only default content is carded.
pub(super) fn selection_and_card(view: &Rc<DomView>, fixture: &Value) {
    let observer = Observer::register("registered");
    let mut renamed = (*tool_call(fixture)).clone();
    renamed.name = "unregistered".into();
    for (call, tool) in [
        (Rc::new(renamed), Some("registered")),
        (tool_call(fixture), Some("")),
        (tool_call(fixture), None),
    ] {
        let row = ToolMessage::<Details> {
            tool_call: Some(call),
            tool: tool.map(str::to_owned),
            ..ToolMessage::new()
        };
        let root = row.mount(view).unwrap();
        assert_eq!(observer.seen.borrow_mut().drain(..).count(), 1);
        assert_eq!(text(&root.node), "native renderer");
    }
    observer.custom.set(false);
    let row = ToolMessage::<Details> {
        tool_call: Some(tool_call(fixture)),
        ..ToolMessage::new()
    };
    let root = row.mount(view).unwrap();
    body().append_child(&root.node).unwrap();
    card_paint(view, &card(&root).unwrap());
    assert_eq!(text(&card(&root).unwrap()), "native renderer");
    // The default renderer's content is ordinary content inside the card.
    let unknown = ToolMessage::<Details> {
        tool_call: Some(tool_call(fixture)),
        tool: Some("unknown".into()),
        ..ToolMessage::new()
    };
    let default = unknown.mount(view).unwrap();
    select(&card(&default).unwrap(), ".maestro-tool-default");
    assert!(text(&default.node).contains("Tool Call"));
    root.node.unchecked_ref::<Element>().remove();
    assert!(ToolMessage::<Details>::new().mount(view).is_err());
    maestro_web::tools::renderer_registry::TOOL_RENDERERS
        .with(|all| all.borrow_mut().shift_remove("registered"));
}
/// The ordinary card paints the brand panel, border, control radius and a small shadow.
fn card_paint(view: &DomView, card: &Element) {
    let colors = view.pack.presentation(view.mode).unwrap().colors;
    assert_eq!(computed(card, "background-color"), rgb(colors["panel"]));
    assert_eq!(computed(card, "border-top-color"), rgb(colors["border"]));
    // One CSS pixel snaps to whole device pixels at the page's scale factor.
    assert_ne!(computed(card, "border-top-width"), "0px");
    assert_eq!(computed(card, "padding-left"), "10px");
    let radius = view.pack.presentation(view.mode).unwrap().radii["control"];
    assert_eq!(
        computed(card, "border-top-left-radius"),
        format!("{radius}px")
    );
    assert_ne!(computed(card, "box-shadow"), "none");
}

/// One listing's original copy operand, painted text and language label.
pub(super) fn listings(root: &DomRoot) -> Vec<(String, String, String)> {
    let blocks = root
        .node
        .unchecked_ref::<Element>()
        .query_selector_all(".maestro-code")
        .unwrap();
    (0..blocks.length())
        .map(|index| {
            let block: Element = blocks.item(index).unwrap().unchecked_into();
            (
                select(&block, "copy-button").get_attribute("text").unwrap(),
                text(&select(&block, "pre code")),
                block.get_attribute("language").unwrap(),
            )
        })
        .collect()
}
/// A recorded value, with the recorder's absent and negative-zero markers restored.
fn recorded(value: &Value) -> Option<Value> {
    Some(match value {
        Value::Object(members)
            if members.len() == 1 && members.get("undefined") == Some(&json!(true)) =>
        {
            return None;
        }
        Value::Object(members)
            if members.len() == 1 && members.get("negativeZero") == Some(&json!(true)) =>
        {
            json!(-0.0)
        }
        Value::Object(members) => Value::Object(
            members
                .iter()
                .map(|(key, member)| (key.clone(), recorded(member).unwrap()))
                .collect(),
        ),
        Value::Array(items) => {
            Value::Array(items.iter().map(|item| recorded(item).unwrap()).collect())
        }
        other => other.clone(),
    })
}
/// A plain JSON result with `content` and optional `details`.
fn debug_result(content: &Value, details: Option<Value>) -> Rc<ToolResultMessage> {
    Rc::new(ToolResultMessage {
        tool_call_id: "call".into(),
        tool_name: "tool".into(),
        content: serde_json::from_value(content.clone()).unwrap(),
        details,
        is_error: false,
        timestamp: 0.,
        usage: None,
    })
}
/// Mount one debug view and compare every listing with its recorded code and painted text.
fn observe_debug(
    view: &DomView,
    row: &ToolMessageDebugView,
    expected: &[Value],
    index: usize,
) -> DomRoot {
    let root = row.mount(view).unwrap();
    body().append_child(&root.node).unwrap();
    let shown = listings(&root);
    let recorded: Vec<_> = expected
        .iter()
        .map(|code| {
            (
                code["code"].as_str().unwrap().to_owned(),
                code["displayed"].as_str().unwrap().to_owned(),
                code["language"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(shown, recorded, "{index}");
    root
}
/// The 32 locale and details cases keep localized labels, independent result visibility and
/// missing, null and literal values distinct.
pub(super) fn debug_cases(view: &Rc<DomView>, fixture: &Value) {
    let cases = fixture["debug"].as_array().unwrap();
    assert_eq!(cases.len(), 32);
    for (index, case) in cases.iter().enumerate() {
        let language = case["input"]["language"].as_str().unwrap();
        let localized = localized_view(view, language);
        let row = ToolMessageDebugView {
            call_args: recorded(&fixture["debugCall"]),
            result: Some(debug_result(
                &fixture["debugContent"],
                case["input"].get("details").cloned(),
            )),
            has_result: case["input"]["hasResult"].as_bool().unwrap(),
        };
        let expected = &case["expected"];
        let root = observe_debug(
            &localized,
            &row,
            expected["code"].as_array().unwrap(),
            index,
        );
        let element: &Element = root.node.unchecked_ref();
        let labels: Vec<_> = (0..2)
            .map(|at| {
                text(
                    &element
                        .query_selector_all(".maestro-tool-section-label")
                        .unwrap()
                        .item(at)
                        .unwrap(),
                )
            })
            .collect();
        assert_eq!(json!(labels), expected["labels"], "{index}");
        let absent = element.query_selector(".maestro-tool-detail").unwrap();
        assert_eq!(
            absent.map(|absent| text(&absent)),
            expected["noResult"]
                .as_bool()
                .unwrap()
                .then(|| fixture["absent"][language].as_str().unwrap().to_owned()),
            "{index}"
        );
        element.remove();
    }
}
/// The 25 recorded values keep strict string parsing, canonical pretty JSON and literal fallbacks
/// in both the call and details listings; the six output cases keep only ordered text blocks.
pub(super) fn pretty_and_output_cases(view: &Rc<DomView>, fixture: &Value) {
    let cases = fixture["pretty"].as_array().unwrap();
    assert_eq!(cases.len(), 25);
    for (index, case) in cases.iter().enumerate() {
        let value = recorded(&case["input"]);
        let content = case["expected"]["content"].as_str().unwrap();
        let language = if case["expected"]["isJson"].as_bool().unwrap() {
            "json"
        } else {
            "text"
        };
        let row = ToolMessageDebugView {
            call_args: value.clone(),
            result: Some(debug_result(&json!([]), value)),
            has_result: true,
        };
        let expected = [
            json!({"code": content, "displayed": content, "language": "json"}),
            json!({"code": "", "displayed": "", "language": "text"}),
            json!({"code": content, "displayed": content, "language": language}),
        ];
        observe_debug(view, &row, &expected, index)
            .node
            .unchecked_ref::<Element>()
            .remove();
    }
    let outputs = fixture["debugOutputs"].as_array().unwrap();
    assert_eq!(outputs.len(), 6);
    for (index, case) in outputs.iter().enumerate() {
        let content = &case["input"]["content"];
        let row = ToolMessageDebugView {
            call_args: Some(json!("YWJj")),
            result: (!content.is_null()).then(|| debug_result(content, Some(Value::Null))),
            has_result: true,
        };
        let expected = case["expected"]["code"].as_array().unwrap();
        observe_debug(view, &row, expected, index)
            .node
            .unchecked_ref::<Element>()
            .remove();
    }
}

/// Tool rows keep their containing root and unchanged content across updates and reconnection,
/// replace only on a changed card choice, keep the last published content on a failed candidate,
/// stay independent of each other and release content when dropped.
pub(super) async fn tool_row_lifecycle(
    view: &Rc<DomView>,
    fixture: &Value,
    page: &wasm_bindgen::JsValue,
) {
    let observer = Observer::register("lifecycle");
    let mut lifecycle = (*tool_call(fixture)).clone();
    lifecycle.name = "lifecycle".into();
    let row = ToolMessage::<Details> {
        tool_call: Some(Rc::new(lifecycle)),
        ..ToolMessage::new()
    };
    let root = row.mount(view).unwrap();
    body().append_child(&root.node).unwrap();
    let other = row.mount(view).unwrap();
    body().append_child(&other.node).unwrap();
    let shown = root.node.first_child().unwrap();
    row.update(&root, view).unwrap();
    assert!(root.node.first_child().unwrap().is_same_node(Some(&shown)));
    root.node.unchecked_ref::<Element>().remove();
    body().append_child(&root.node).unwrap();
    *observer.text.borrow_mut() = "current text";
    row.update(&root, view).unwrap();
    assert!(root.node.first_child().unwrap().is_same_node(Some(&shown)));
    assert_eq!(text(&root.node), "current text");
    assert_eq!(text(&other.node), "native renderer");
    native_text(page, &root.node).await;
    assert_eq!(observer.released.get(), 0);
    card_choice_and_failures(view, &row, &root, &observer);
    root.node.unchecked_ref::<Element>().remove();
    drop(root);
    assert_eq!(observer.released.get(), 2);
    assert_eq!(text(&other.node), "native renderer");
    other.node.unchecked_ref::<Element>().remove();
    drop(other);
    assert_eq!(observer.released.get(), 3);
    maestro_web::tools::renderer_registry::TOOL_RENDERERS
        .with(|all| all.borrow_mut().shift_remove("lifecycle"));
}
/// The page's own find and selection reach the row's text.
async fn native_text(page: &wasm_bindgen::JsValue, root: &web_sys::Node) {
    assert!(
        call(page, "find", &js_sys::Array::of1(&"current text".into()))
            .await
            .as_bool()
            .unwrap()
    );
    let selected = call(page, "select", &js_sys::Array::of1(root)).await;
    assert_eq!(selected.as_string().unwrap(), "current text");
}
/// A changed card choice replaces the content once; failed candidates keep the published card.
fn card_choice_and_failures(
    view: &DomView,
    row: &ToolMessage<Details>,
    root: &DomRoot,
    observer: &Observer,
) {
    observer.custom.set(false);
    row.update(root, view).unwrap();
    assert_eq!(observer.released.get(), 1);
    let carded = card(root).unwrap();
    assert_eq!(text(&carded), "current text");
    for failure in [Output::Refused, Output::FailingMount] {
        observer.output.set(failure);
        observer.custom.set(true);
        assert!(row.update(root, view).is_err());
        assert!(card(root).unwrap().is_same_node(Some(&carded)));
        assert_eq!(text(&root.node), "current text");
    }
    observer.output.set(Output::Tracked);
}
/// Debug listings stay in place while the result stays shown and are rebuilt only when it returns.
pub(super) fn debug_row_lifecycle(view: &Rc<DomView>) {
    let mut row = ToolMessageDebugView {
        call_args: Some(json!({"a": 1})),
        result: Some(debug_result(
            &json!([{"type": "text", "text": "out"}]),
            None,
        )),
        has_result: true,
    };
    let root = row.mount(view).unwrap();
    body().append_child(&root.node).unwrap();
    let element: &Element = root.node.unchecked_ref();
    let first = element.query_selector_all(".maestro-code").unwrap();
    assert_eq!(first.length(), 3);
    row.call_args = Some(json!({"a": 2}));
    row.update(&root, view).unwrap();
    let again = element.query_selector_all(".maestro-code").unwrap();
    for at in 0..3 {
        assert!(
            again
                .item(at)
                .unwrap()
                .is_same_node(first.item(at).as_ref())
        );
    }
    assert_eq!(listings(&root)[0].1, "{\n  \"a\": 2\n}");
    row.has_result = false;
    row.update(&root, view).unwrap();
    assert_eq!(
        element
            .query_selector_all(".maestro-code")
            .unwrap()
            .length(),
        1
    );
    assert_eq!(
        text(&select(element, ".maestro-tool-detail")),
        "(no result)"
    );
    root.node.unchecked_ref::<Element>().remove();
    body().append_child(&root.node).unwrap();
    row.has_result = true;
    row.update(&root, view).unwrap();
    assert!(
        element
            .query_selector(".maestro-tool-detail")
            .unwrap()
            .is_none()
    );
    assert_eq!(listings(&root).len(), 3);
    assert!(
        element
            .query_selector_all(".maestro-code")
            .unwrap()
            .item(0)
            .unwrap()
            .is_same_node(first.item(0).as_ref())
    );
    element.remove();
}
/// The abort row keeps one localized span, updated in place.
pub(super) fn abort_row(view: &Rc<DomView>, fixture: &Value) {
    let row = AbortedMessage;
    let root = row.mount(view).unwrap();
    body().append_child(&root.node).unwrap();
    let element: &Element = root.node.unchecked_ref();
    assert_eq!(element.tag_name(), "SPAN");
    assert_eq!(text(&root.node), fixture["aborted"]["en"].as_str().unwrap());
    assert_eq!(computed(element, "font-style"), "italic");
    let colors = view.pack.presentation(view.mode).unwrap().colors;
    assert_eq!(computed(element, "color"), rgb(colors["error"]));
    row.update(&root, &localized_view(view, "de")).unwrap();
    assert_eq!(text(&root.node), fixture["aborted"]["de"].as_str().unwrap());
    assert!(root.node.is_connected());
    element.remove();
}
