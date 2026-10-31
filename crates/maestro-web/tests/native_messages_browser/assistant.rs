//! Assistant rows through actual native children and application-owned records.
use crate::contributions_support::{body, select, text};
use maestro_app::{JsonObject, SharedAssistantMessage, ToolResultMessage};
use maestro_web::{
    components::messages::{AssistantMessage, ToolResults},
    dom::{DomContent, DomRoot, DomView},
    tools::types::{
        ToolRenderResult, ToolRenderer, ToolRendererBinding, ToolResultMessageView, ToolUiError,
    },
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{Arc, RwLock},
};
use wasm_bindgen::JsCast as _;
use web_sys::{Element, HtmlElement};

/// One standard renderer invocation, after the assistant has selected its operands.
#[derive(Debug)]
struct Seen {
    /// Reached argument object.
    params: Value,
    /// Name of the registration actually reached.
    selected: String,
    /// Exact result identity, error flag, timestamp and text content.
    result: Option<(String, bool, f64, Vec<maestro_app::UserBlock>)>,
    /// Effective tool streaming state.
    streaming: bool,
}
/// Native renderer observing standard envelopes and reentrant application edits.
#[derive(Default)]
struct Recorder {
    /// Calls in dispatch order.
    seen: RefCell<Vec<Seen>>,
    /// Name under which this callback is registered.
    selected: String,
    /// Optional reentrant effect before observing the selected envelope.
    before: RefCell<Option<Rc<dyn Fn()>>>,
    /// Whether dispatch refuses its reached input.
    fail: Cell<bool>,
}
impl Recorder {
    /// Replace one native registration with an independent observer.
    fn register(name: &str) -> Rc<Self> {
        let recorder = Rc::new(Self {
            selected: name.into(),
            ..Self::default()
        });
        maestro_web::register_tool_renderer(name, ToolRendererBinding::from_dom(recorder.clone()));
        recorder
    }
}
impl ToolRenderer<JsonObject, Value, Box<dyn DomContent>> for Recorder {
    fn render(
        &self,
        params: Option<&JsonObject>,
        result: Option<ToolResultMessageView<'_>>,
        streaming: bool,
    ) -> Result<ToolRenderResult<Box<dyn DomContent>>, ToolUiError> {
        let before = self.before.borrow().clone();
        if let Some(before) = before {
            before();
        }
        if self.fail.get() {
            return Err(ToolUiError::Presentation("assistant child refused".into()));
        }
        self.seen.borrow_mut().push(Seen {
            selected: self.selected.clone(),
            params: params.map_or(Value::Null, |params| Value::Object(params.clone())),
            result: result.map(|result| {
                (
                    result.tool_call_id().into(),
                    result.is_error(),
                    result.timestamp(),
                    result.content().to_vec(),
                )
            }),
            streaming,
        });
        Ok(ToolRenderResult {
            content: Box::new(Label),
            is_custom: true,
        })
    }
}
/// Searchable native content from the selected custom renderer.
struct Label;
impl DomContent for Label {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let node = view.document.create_element("p").unwrap();
        node.set_text_content(Some("selected native tool"));
        Ok(DomRoot::new(node))
    }
    fn update(&self, _: &DomRoot, _: &DomView) -> Result<(), ToolUiError> {
        Ok(())
    }
}
/// Supply required model metadata and a real required usage record around recorded content.
pub(super) fn message(content: Value, stop: &str) -> SharedAssistantMessage {
    let mut value = json!({
        "role":"assistant", "content":[], "api":"controlled", "provider":"controlled", "model":"controlled",
        "usage":maestro_app::Usage::default(), "stopReason":stop, "timestamp":0,
    });
    value["content"] = content;
    Arc::new(RwLock::new(serde_json::from_value(value).unwrap()))
}
/// Original standard result with independently supplied ID and timestamp.
fn result(id: &str, timestamp: f64) -> Arc<RwLock<ToolResultMessage>> {
    Arc::new(RwLock::new(serde_json::from_value(json!({
        "role":"toolResult", "toolCallId":id, "toolName":"registered", "content":[], "isError":false, "timestamp":timestamp,
    })).unwrap()))
}
/// Replay the source's seven admission records and thirty-two independent tool policies.
pub(super) fn visibility(view: &DomView, fixture: &Value) {
    let recorder = Recorder::register("registered");
    assert_eq!(fixture["assistant"].as_array().unwrap().len(), 75);
    for case in fixture["assistant"].as_array().unwrap().iter().take(39) {
        visibility_case(view, fixture, case, &recorder);
    }
    names(view, fixture, &recorder);
    retained_positions(view, &recorder);
    exact_results_and_reentrant_reads(view);
}
/// One recorded content admission and its reached native tool operands.
fn visibility_case(view: &DomView, fixture: &Value, case: &Value, recorder: &Recorder) {
    let input = &case["input"];
    let parts = case["expected"]["parts"].as_array().unwrap();
    let content = input
        .get("content")
        .cloned()
        .unwrap_or_else(|| json!([fixture["call"]]));
    let stop = if input["aborted"] == true {
        "aborted"
    } else {
        input["stopReason"].as_str().unwrap_or("toolUse")
    };
    let message = message(content, stop);
    let mut row = AssistantMessage::<Value> {
        message: Some(message),
        hide_tool_calls: input["hide"] == true,
        hide_pending_tool_calls: input["hidePending"] == true,
        is_streaming: false,
        ..AssistantMessage::default()
    };
    if input["pending"] == true {
        row.pending_tool_calls = Some(Rc::new(HashSet::from(["call".into()])));
    }
    if input["result"] == true {
        row.tool_results_by_id = Some(ToolResults::Live(Rc::new(HashMap::from([(
            "call".into(),
            result("call", 0.),
        )]))));
    }
    recorder.seen.borrow_mut().clear();
    let root = row.mount(view).unwrap();
    let node: &Element = root.node.unchecked_ref();
    assert_eq!(
        select(node, ".maestro-assistant-parts").child_element_count() as usize,
        parts.len(),
        "{input}"
    );
    observed_tool(
        recorder,
        parts.first().filter(|part| part["kind"] == "tool"),
    );
    assert_eq!(
        optional_text(node, ".maestro-message-abort"),
        case["expected"]["abort"].as_str().map(str::to_owned)
    );
    assert_eq!(optional_text(node, ".maestro-message-error"), None);
    assert_sources(node, parts);
}
/// Native text and thinking consumers receive the selected source and streaming state.
fn assert_sources(node: &Element, parts: &[Value]) {
    for part in parts.iter().filter(|part| part["kind"] == "text") {
        assert_eq!(
            text(&select(node, ".maestro-markdown")).trim(),
            part["source"].as_str().unwrap().trim()
        );
    }
    for part in parts.iter().filter(|part| part["kind"] == "thinking") {
        let thinking = select(node, ".maestro-thinking-block");
        assert_eq!(
            thinking
                .query_selector(".maestro-shimmer")
                .unwrap()
                .is_some(),
            part["streaming"].as_bool().unwrap()
        );
        select(&thinking, "button")
            .unchecked_into::<HtmlElement>()
            .click();
        assert_eq!(
            text(&select(&thinking, ".maestro-markdown")).trim(),
            part["source"].as_str().unwrap().trim()
        );
    }
}
/// Reached tool operands observed at the actual selected registration.
fn observed_tool(recorder: &Recorder, part: Option<&Value>) {
    if let Some(part) = part {
        let seen = recorder.seen.borrow();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].params, part["call"]["arguments"]);
        assert_eq!(
            seen[0].streaming,
            (part["pending"] == true || part["streaming"] == true) && part["aborted"] != true
        );
        if part["aborted"] == true {
            let result = seen[0].result.as_ref().unwrap();
            assert_eq!((&*result.0, result.1, result.3.len()), ("call", true, 0));
        } else if part["result"].get("undefined").is_some() {
            assert!(seen[0].result.is_none());
        } else {
            assert_eq!(
                seen[0]
                    .result
                    .as_ref()
                    .map(|result| (&*result.0, result.1, result.2)),
                Some(("call", false, 0.))
            );
        }
    } else {
        assert!(recorder.seen.borrow().is_empty());
    }
}
/// All five original tool-name producers reach the already-selected dispatcher name.
fn names(view: &DomView, fixture: &Value, recorder: &Recorder) {
    let presentation = Rc::new(crate::presentation::Presentation::new(Rc::new(
        crate::attachment_support::Host::default(),
    )));
    let panel =
        maestro_app::browser::tools::artifacts::LocalArtifactsPanel::new_local(presentation);
    for case in fixture["assistantNames"].as_array().unwrap() {
        let tools = case["tools"].as_array().map(|names| {
            Rc::new(RwLock::new(
                names
                    .iter()
                    .map(|name| {
                        let mut tool = panel.tool();
                        tool.definition.name = name["name"].as_str().unwrap().into();
                        Rc::new(RwLock::new(tool))
                    })
                    .collect(),
            ))
        });
        let row = AssistantMessage::<Value> {
            message: Some(message(json!([fixture["call"]]), "toolUse")),
            tools: tools.clone(),
            ..AssistantMessage::default()
        };
        let _exclusive = tools.as_ref().map(|tools| tools.write().unwrap());
        recorder.seen.borrow_mut().clear();
        let root = row.mount(view).unwrap();
        assert_eq!(recorder.seen.borrow().len(), 1, "{case}");
        assert_eq!(
            recorder.seen.borrow()[0].selected,
            case["selected"].as_str().unwrap()
        );
        assert!(
            root.node
                .text_content()
                .unwrap()
                .contains("selected native tool")
        );
    }
}
/// Emitted position, not source position, retains reasoning expansion and original source bytes.
fn retained_positions(view: &DomView, recorder: &Recorder) {
    let message = message(
        json!([
            {"type":"thinking","thinking":"first"}, {"type":"thinking","thinking":"second"},
            {"type":"text","text":"    indentation is content"}, {"type":"text","text":"same"}, {"type":"text","text":"same"},
        ]),
        "stop",
    );
    let row = AssistantMessage::<Value> {
        message: Some(message.clone()),
        ..AssistantMessage::default()
    };
    let root = row.mount(view).unwrap();
    body().append_child(&root.node).unwrap();
    let node: &Element = root.node.unchecked_ref();
    let first = select(node, ".maestro-thinking-block");
    select(&first, "button")
        .unchecked_into::<HtmlElement>()
        .click();
    assert!(first.class_list().contains("maestro-thinking-expanded"));
    assert_eq!(text(&select(node, "code")), "indentation is content\n");
    assert_eq!(
        node.query_selector_all(".maestro-assistant-parts > .maestro-markdown")
            .unwrap()
            .length(),
        3
    );
    message.write().unwrap().content[0] =
        serde_json::from_value(json!({"type":"thinking","thinking":"\u{feff}"})).unwrap();
    row.update(&root, view).unwrap();
    assert_eq!(select(node, ".maestro-thinking-block"), first);
    assert_eq!(
        text(&select(&first, ".maestro-markdown")).trim_end_matches('\n'),
        "second"
    );
    assert_eq!(
        first.get_attribute("class").unwrap(),
        "maestro-thinking-block maestro-thinking-expanded"
    );
    recorder.fail.set(true);
    message.write().unwrap().content[1] = serde_json::from_value(
        json!({"type":"toolCall","id":"failure","name":"registered","arguments":{}}),
    )
    .unwrap();
    assert!(row.update(&root, view).is_err());
    assert_eq!(select(node, ".maestro-thinking-block"), first);
    assert!(text(&first).contains("second"));
    recorder.fail.set(false);
    node.remove();
    body().append_child(&root.node).unwrap();
    message.write().unwrap().content.clear();
    row.update(&root, view).unwrap();
    assert_eq!(
        select(node, ".maestro-assistant-parts").child_element_count(),
        0
    );
    node.remove();
}
/// Exact IDs and live reached results survive callbacks without holding application guards.
fn exact_results_and_reentrant_reads(view: &DomView) {
    let recorder = Recorder::register("registered");
    let results = Rc::new(
        ["call", "Call", "", "雪", " call"]
            .into_iter()
            .enumerate()
            .map(|(index, id)| {
                (
                    id.to_owned(),
                    result(id, f64::from(u32::try_from(index).unwrap())),
                )
            })
            .collect::<HashMap<_, _>>(),
    );
    let message = message(
        json!([
            {"type":"toolCall","id":"Call","name":"registered","arguments":{"order":0}},
            {"type":"toolCall","id":"call","name":"registered","arguments":{"order":1}},
            {"type":"toolCall","id":"雪","name":"registered","arguments":{}},
            {"type":"toolCall","id":"","name":"registered","arguments":{}},
            {"type":"toolCall","id":" call","name":"registered","arguments":{}},
        ]),
        "toolUse",
    );
    let tools = Rc::new(RwLock::new(Vec::new()));
    let calls = Rc::new(Cell::new(0));
    let (live, current, collection, invoked) =
        (results.clone(), message.clone(), tools.clone(), calls);
    *recorder.before.borrow_mut() = Some(Rc::new(move || {
        assert!(current.try_write().is_ok());
        assert!(collection.try_write().is_ok());
        if invoked.get() == 0 {
            live["call"].write().unwrap().timestamp = 42.;
        }
        invoked.set(invoked.get() + 1);
    }));
    let row = AssistantMessage::<Value> {
        message: Some(message),
        tools: Some(tools),
        tool_results_by_id: Some(ToolResults::Live(results)),
        is_streaming: true,
        ..AssistantMessage::default()
    };
    let _root = row.mount(view).unwrap();
    assert_live_results(&recorder);
}

/// Every selected live result retains its exact ID and the current reached timestamp.
fn assert_live_results(recorder: &Recorder) {
    let seen = recorder.seen.borrow();
    assert_eq!(
        seen.iter()
            .map(|seen| {
                let result = seen.result.as_ref().unwrap();
                (&*result.0, result.2)
            })
            .collect::<Vec<_>>(),
        [
            ("Call", 1.),
            ("call", 42.),
            ("雪", 3.),
            ("", 2.),
            (" call", 4.)
        ]
    );
    assert!(seen.iter().all(|seen| seen.streaming));
}
/// Required model usage populated from the recorder's independent counters.
fn recorded_usage(fixture: &Value) -> maestro_app::Usage {
    let value = &fixture["assistantUsage"];
    maestro_app::Usage {
        input: value["input"].as_f64().unwrap(),
        output: value["output"].as_f64().unwrap(),
        cache_read: value["cacheRead"].as_f64().unwrap(),
        cache_write: value["cacheWrite"].as_f64().unwrap(),
        cost: maestro_app::UsageCost {
            total: value["cost"]["total"].as_f64().unwrap(),
            ..maestro_app::UsageCost::default()
        },
        ..maestro_app::Usage::default()
    }
}
/// Replay all thirty-six independent usage, streaming and outcome records.
pub(super) fn outcomes(view: &DomView, fixture: &Value) {
    for case in fixture["assistant"].as_array().unwrap().iter().skip(39) {
        outcome_case(view, fixture, case);
    }
    current_action(view, fixture);
    localized_outcomes(view, fixture);
}
/// One source-derived usage and outcome selection, including missing and empty errors.
fn outcome_case(view: &DomView, fixture: &Value, case: &Value) {
    let input = &case["input"];
    let message = message(json!([]), input["stopReason"].as_str().unwrap());
    {
        let mut message = message.write().unwrap();
        message.usage = recorded_usage(fixture);
        message.error_message = input["errorMessage"].as_str().map(str::to_owned);
    }
    let row = AssistantMessage::<Value> {
        message: Some(message),
        is_streaming: input["isStreaming"] == true,
        on_cost_click: (input["clickable"] == true).then(|| Rc::new(|| {}) as Rc<dyn Fn()>),
        ..AssistantMessage::default()
    };
    let root = row.mount(view).unwrap();
    let node: &Element = root.node.unchecked_ref();
    assert_eq!(
        optional_text(node, ".maestro-message-usage"),
        case["expected"]["usage"].as_str().map(str::to_owned),
        "{input}"
    );
    let expected = &case["expected"]["error"];
    let expected = expected.as_array().map(|parts| {
        format!(
            "{} {}",
            parts[0].as_str().unwrap(),
            parts[1].as_str().unwrap()
        )
    });
    assert_eq!(
        optional_text(node, ".maestro-message-error"),
        expected,
        "{input}"
    );
    assert_eq!(
        optional_text(node, ".maestro-message-abort"),
        case["expected"]["abort"].as_str().map(str::to_owned),
        "{input}"
    );
}
/// Text of a selected optional native element.
fn optional_text(node: &Element, selector: &str) -> Option<String> {
    node.query_selector(selector)
        .unwrap()
        .map(|node| text(&node))
}
/// A cost action using the caller's original count owner.
fn counting_action(count: &Rc<Cell<u32>>) -> Rc<dyn Fn()> {
    let count = Rc::clone(count);
    Rc::new(move || count.set(count.get() + 1))
}
/// Current invocation counts at one recorded interaction.
fn assert_click_counts(first: &Cell<u32>, second: &Cell<u32>, expected: &Value) {
    assert_eq!(
        u64::from(first.get()),
        expected["firstClicks"].as_u64().unwrap()
    );
    assert_eq!(
        u64::from(second.get()),
        expected["secondClicks"].as_u64().unwrap()
    );
}
/// Replacement, removal, streaming and release always use the current cost action.
fn current_action(view: &DomView, fixture: &Value) {
    let message = message(json!([]), "stop");
    message.write().unwrap().usage = recorded_usage(fixture);
    let (first, second) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let mut row = AssistantMessage::<Value> {
        message: Some(message.clone()),
        on_cost_click: Some(counting_action(&first)),
        ..AssistantMessage::default()
    };
    let root = row.mount(view).unwrap();
    body().append_child(&root.node).unwrap();
    replace_action(view, &mut row, &root, (&first, &second), fixture);
    let node: &Element = root.node.unchecked_ref();
    row.on_cost_click = Some(counting_action(&second));
    row.update(&root, view).unwrap();
    let current: HtmlElement = select(node, "button.maestro-message-usage").unchecked_into();
    row.is_streaming = true;
    row.update(&root, view).unwrap();
    current.click();
    let expected = &fixture["assistantInteractions"][2]["expected"];
    assert_eq!(
        optional_text(node, ".maestro-message-usage").is_some(),
        expected["usageVisible"].as_bool().unwrap()
    );
    assert_eq!((first.get(), second.get()), (1, 1));
    row.is_streaming = false;
    message.write().unwrap().usage = maestro_app::Usage::default();
    row.update(&root, view).unwrap();
    assert_eq!(text(&select(node, ".maestro-message-usage")), "");
    let current: HtmlElement = select(node, "button.maestro-message-usage").unchecked_into();
    node.remove();
    drop(root);
    current.click();
    assert_eq!((first.get(), second.get()), (1, 1));
}
/// An unchanged button uses the replaced action; a plain replacement revokes the old button.
fn replace_action(
    view: &DomView,
    row: &mut AssistantMessage,
    root: &DomRoot,
    (first, second): (&Rc<Cell<u32>>, &Rc<Cell<u32>>),
    fixture: &Value,
) {
    let node: &Element = root.node.unchecked_ref();
    let expected = &fixture["assistantInteractions"];
    let button: HtmlElement = select(node, "button.maestro-message-usage").unchecked_into();
    assert_eq!(
        text(button.unchecked_ref()),
        expected[0]["expected"]["text"].as_str().unwrap()
    );
    button.click();
    assert_click_counts(first, second, &expected[0]["expected"]);
    row.on_cost_click = Some(counting_action(second));
    row.update(root, view).unwrap();
    assert_eq!(
        select(node, "button.maestro-message-usage"),
        button.clone().unchecked_into::<Element>()
    );
    button.click();
    assert_click_counts(first, second, &expected[1]["expected"]);
    row.on_cost_click = None;
    row.update(root, view).unwrap();
    button.click();
    assert_eq!((first.get(), second.get()), (1, 1));
    let expected = &expected[3]["expected"];
    assert_eq!(
        expected["clickable"].as_bool().unwrap(),
        node.query_selector("button.maestro-message-usage")
            .unwrap()
            .is_some()
    );
    assert_eq!(
        text(&select(node, ".maestro-message-usage")),
        expected["text"].as_str().unwrap()
    );
}
/// Localized outcomes stay literal and remain selected independently of streaming usage.
fn localized_outcomes(view: &DomView, fixture: &Value) {
    let message = message(json!([]), "error");
    message.write().unwrap().error_message = Some("<b>literal & error</b>".into());
    let row = AssistantMessage::<Value> {
        message: Some(message.clone()),
        is_streaming: true,
        ..AssistantMessage::default()
    };
    let root = row.mount(view).unwrap();
    let node: &Element = root.node.unchecked_ref();
    for case in fixture["assistantStrings"].as_array().unwrap() {
        let view =
            crate::contributions_support::localized_view(view, case["language"].as_str().unwrap());
        message.write().unwrap().stop_reason = maestro_app::StopReason::Error;
        row.update(&root, &view).unwrap();
        assert_eq!(
            text(&select(node, ".maestro-message-error")),
            case["errorText"].as_str().unwrap()
        );
        assert_eq!(
            text(&select(node, ".maestro-message-error strong")),
            case["errorPrefix"].as_str().unwrap()
        );
        assert_eq!(
            u64::from(node.query_selector_all("b").unwrap().length()),
            case["errorElements"].as_u64().unwrap()
        );
        message.write().unwrap().stop_reason = maestro_app::StopReason::Aborted;
        row.update(&root, &view).unwrap();
        assert!(optional_text(node, ".maestro-message-error").is_none());
        assert_eq!(
            text(&select(node, ".maestro-message-abort")),
            case["aborted"].as_str().unwrap()
        );
        assert!(optional_text(node, ".maestro-message-usage").is_none());
    }
}

/// Assistant roots retain native envelopes, reconnect independently and revoke their children.
pub(super) fn lifecycle(view: &DomView, fixture: &Value) {
    let observer = super::rows::Observer::register("registered");
    let details = Rc::new(Cell::new(7));
    let result = super::rows::tool_result(fixture, &details);
    let message = message(json!([fixture["call"]]), "toolUse");
    let mut row = AssistantMessage::<super::rows::Details> {
        message: Some(message),
        tool_results_by_id: Some(ToolResults::Native(Rc::new(HashMap::from([(
            "call".into(),
            result,
        )])))),
        ..AssistantMessage::default()
    };
    let root = row.mount(view).unwrap();
    let other = row.mount(view).unwrap();
    body().append_child(&root.node).unwrap();
    body().append_child(&other.node).unwrap();
    {
        let seen = observer.seen.borrow();
        assert_eq!(seen.len(), 2);
        for call in &*seen {
            assert!(Rc::ptr_eq(
                call.result.as_ref().unwrap().5.as_ref().unwrap(),
                &details
            ));
        }
    }
    lifecycle_updates(view, &mut row, &root, &other, &observer);
    root.node.unchecked_ref::<Element>().remove();
    other.node.unchecked_ref::<Element>().remove();
    drop(root);
    drop(other);
    assert_eq!(observer.released.get(), 2);
    native_details_without_bounds(view);
}
/// A failed kind replacement and an absent required record keep the last published row.
fn lifecycle_updates(
    view: &DomView,
    row: &mut AssistantMessage<super::rows::Details>,
    root: &DomRoot,
    other: &DomRoot,
    observer: &super::rows::Observer,
) {
    let node: &Element = root.node.unchecked_ref();
    let second: &Element = other.node.unchecked_ref();
    let child = select(node, ".maestro-tool-message p");
    let independent = select(second, ".maestro-tool-message p");
    row.update(root, view).unwrap();
    assert_eq!(select(node, ".maestro-tool-message p"), child);
    node.remove();
    body().append_child(&root.node).unwrap();
    row.update(root, view).unwrap();
    assert_eq!(select(node, ".maestro-tool-message p"), child);
    let message = row.message.as_ref().unwrap().clone();
    message.write().unwrap().content =
        serde_json::from_value(json!([{"type":"thinking","thinking":"kept reasoning"}])).unwrap();
    row.update(root, view).unwrap();
    let thinking = select(node, ".maestro-thinking-block");
    observer.output.set(super::rows::Output::FailingMount);
    message.write().unwrap().content = serde_json::from_value(
        json!([{"type":"toolCall","id":"call","name":"registered","arguments":{}}]),
    )
    .unwrap();
    assert!(row.update(root, view).is_err());
    assert_eq!(select(node, ".maestro-thinking-block"), thinking);
    assert_eq!(select(second, ".maestro-tool-message p"), independent);
    row.message = None;
    assert!(row.update(root, view).is_err());
    assert_eq!(select(node, ".maestro-thinking-block"), thinking);
}
/// Native details need no cloning, serialization, deserialization or default implementation.
fn native_details_without_bounds(view: &DomView) {
    /// Opaque application detail with no derived transport or ownership traits.
    struct Opaque;
    let call = json!({"type":"toolCall","id":"opaque","name":"native-opaque","arguments":{}});
    let result = Rc::new(ToolResultMessage {
        tool_call_id: "opaque".into(),
        tool_name: "native-opaque".into(),
        content: Vec::new(),
        details: Some(Opaque),
        is_error: false,
        timestamp: 0.,
    });
    let mut row = AssistantMessage::<Opaque>::default();
    assert!(row.mount(view).is_err());
    row.message = Some(message(json!([call]), "toolUse"));
    row.tool_results_by_id = Some(ToolResults::Native(Rc::new(HashMap::from([(
        "opaque".into(),
        result,
    )]))));
    let root = row.mount(view).unwrap();
    assert!(root.node.text_content().unwrap().contains("Tool Call"));
}
