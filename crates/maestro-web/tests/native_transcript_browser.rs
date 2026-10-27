//! Stable transcript history and the streaming message mounted as page content.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{body, text, view};
use maestro_app::browser::components::messages::CustomAgentMessages as BrowserMessages;
use maestro_app::{AgentMessage, CustomAgentMessages, JsonObject, ToolResultMessage};
use maestro_web::{
    MessageList, MessageRenderer, StreamingMessageContainer,
    dom::{DomContent, DomRoot, DomView},
    register_message_renderer, register_tool_renderer,
    tools::types::{
        ToolRenderResult, ToolRenderer, ToolRendererBinding, ToolResultMessageView, ToolUiError,
    },
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    rc::Rc,
    sync::{Arc, RwLock},
};
use wasm_bindgen::{JsCast as _, prelude::wasm_bindgen};
use web_sys::{Element, HtmlElement};

/// A caller-defined message with an open role.
struct Notice {
    /// Selected role.
    role: String,
    /// Text a registered renderer shows.
    text: String,
}
impl CustomAgentMessages for Notice {
    fn role(&self) -> &str {
        &self.role
    }
}
/// One transcript entry of the test history.
type Entry = AgentMessage<BrowserMessages<Notice>>;

/// Page content showing owned text.
struct Label(String);
impl DomContent for Label {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let node = view.document.create_element("p").unwrap();
        node.set_text_content(Some(&self.0));
        Ok(DomRoot::new(node))
    }
    fn update(&self, root: &DomRoot, _: &DomView) -> Result<(), ToolUiError> {
        root.node.set_text_content(Some(&self.0));
        Ok(())
    }
}

/// Prepare page content for one entry.
type Render = dyn Fn(&Entry) -> Result<Box<dyn DomContent>, ToolUiError>;
/// A role renderer backed by a closure.
struct Role(Box<Render>);
impl MessageRenderer<BrowserMessages<Notice>, Box<dyn DomContent>> for Role {
    fn render(&self, message: &Entry) -> Result<Box<dyn DomContent>, ToolUiError> {
        (self.0)(message)
    }
}
/// Register `render` for `role` in the test message table.
fn role(name: &str, render: impl Fn(&Entry) -> Result<Box<dyn DomContent>, ToolUiError> + 'static) {
    register_message_renderer::<BrowserMessages<Notice>, Box<dyn DomContent>>(
        name,
        Rc::new(Role(Box::new(render))),
    );
}

/// One tool call as the selected renderer received it.
#[derive(Debug, PartialEq)]
struct Seen {
    /// Exact call ID of the selected result, if any.
    id: Option<String>,
    /// Details of the selected result.
    details: Option<Value>,
    /// Streaming flag the row passed on.
    streaming: bool,
}
/// The `probe` tool renderer, recording each call.
#[derive(Default)]
struct Probe {
    /// Calls in dispatch order.
    seen: RefCell<Vec<Seen>>,
}
impl ToolRenderer<JsonObject, Value, Box<dyn DomContent>> for Probe {
    fn render(
        &self,
        _: Option<&JsonObject>,
        result: Option<ToolResultMessageView<'_>>,
        streaming: bool,
    ) -> Result<ToolRenderResult<Box<dyn DomContent>>, ToolUiError> {
        self.seen.borrow_mut().push(Seen {
            id: result
                .as_ref()
                .map(|result| result.tool_call_id().to_owned()),
            details: result.as_ref().and_then(|result| result.details().cloned()),
            streaming,
        });
        Ok(ToolRenderResult {
            content: Box::new(Label("probe".into())),
            is_custom: true,
        })
    }
}
/// Register a fresh `probe` renderer.
fn probe() -> Rc<Probe> {
    let probe = Rc::new(Probe::default());
    register_tool_renderer("probe", ToolRendererBinding::from_dom(probe.clone()));
    probe
}

/// An assistant entry with `content`, nonzero usage and a settled stop reason.
fn assistant(content: Value) -> Entry {
    let usage = json!({"input":3,"output":4,"cacheRead":0,"cacheWrite":0,"totalTokens":7,
        "cost":{"input":0.25,"output":0.5,"cacheRead":0,"cacheWrite":0,"total":0.75}});
    let mut message = json!({
        "role":"assistant", "content":[], "api":"controlled", "provider":"controlled",
        "model":"controlled", "usage":usage, "stopReason":"stop", "timestamp":0,
    });
    message["content"] = content;
    AgentMessage::Assistant(Arc::new(RwLock::new(
        serde_json::from_value(message).unwrap(),
    )))
}
/// An assistant entry calling `probe` once per ID.
fn calling(ids: &[&str]) -> Entry {
    let calls: Vec<_> = (ids.iter())
        .map(|id| json!({"type":"toolCall","id":id,"name":"probe","arguments":{}}))
        .collect();
    assistant(Value::Array(calls))
}
/// A result envelope for `id` carrying `details`.
fn result(id: &str, details: Value) -> Arc<RwLock<ToolResultMessage>> {
    Arc::new(RwLock::new(ToolResultMessage {
        tool_call_id: id.into(),
        tool_name: "probe".into(),
        content: Vec::new(),
        details: Some(details),
        is_error: false,
        timestamp: 0.,
    }))
}
/// An ordinary user entry.
fn user(text: &str) -> Entry {
    let message = json!({"role":"user","content":text,"timestamp":0});
    AgentMessage::User(Arc::new(RwLock::new(
        serde_json::from_value(message).unwrap(),
    )))
}
/// A user entry with one attachment.
fn attached(text: &str) -> Entry {
    let message = json!({"role":"user-with-attachments","content":text,"timestamp":0,
        "attachments":[{"id":"a","type":"document","fileName":"notes.txt","content":"",
        "mimeType":"text/plain","size":0}]});
    let message = BrowserMessages::UserWithAttachments(serde_json::from_value(message).unwrap());
    AgentMessage::Custom(Arc::new(RwLock::new(message)))
}
/// A persistence-only artifact entry.
fn artifact() -> Entry {
    let message = json!({"role":"artifact","action":"create","filename":"a.txt","content":"x",
        "timestamp":"0"});
    let message = BrowserMessages::Artifact(serde_json::from_value(message).unwrap());
    AgentMessage::Custom(Arc::new(RwLock::new(message)))
}
/// A caller-defined entry with `role`.
fn notice(role: &str, text: &str) -> Entry {
    let message = BrowserMessages::Other(Notice {
        role: role.into(),
        text: text.into(),
    });
    AgentMessage::Custom(Arc::new(RwLock::new(message)))
}

/// The recorded transcript fixture.
fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/native_transcript/messages.json")).unwrap()
}

/// A list over `history`, mounted under the page body.
fn mounted(view: &DomView, history: Vec<Entry>) -> (MessageList<Notice>, DomRoot) {
    let list = MessageList::<Notice> {
        messages: Arc::new(RwLock::new(history)),
        ..MessageList::default()
    };
    let root = list.mount(view).unwrap();
    body().append_child(&root.node).unwrap();
    (list, root)
}
/// The list's row roots in order.
fn rows(root: &DomRoot) -> Vec<Element> {
    let children = root.node.child_nodes();
    (0..children.length())
        .filter_map(|index| children.item(index).unwrap().dyn_into().ok())
        .collect()
}
/// Remove a mounted list from the page.
fn unmount(root: &DomRoot) {
    root.node.unchecked_ref::<Element>().remove();
}

/// Exact result IDs pair the last envelope, artifacts never reach role renderers, registered
/// roles take precedence with the original handle, other entries fall back to the built-in
/// rows, registered empty content stays selected and a renderer error propagates.
#[wasm_bindgen]
pub fn message_list_pairs_last_results_and_filters_artifacts() {
    let view = view();
    let (_, root) = mounted(&view, Vec::new());
    assert!(rows(&root).is_empty());
    unmount(&root);
    let fixture = fixture();
    exact_ids(&view, &fixture["listExactIds"]);
    fallbacks(&view);
    precedence(&view);
}

/// Each recorded ID, including empty, Unicode and case-distinct ones, selects the last
/// result for that exact ID and not its decoy.
fn exact_ids(view: &DomView, records: &Value) {
    assert_eq!(records.as_array().unwrap().len(), 5);
    for row in records.as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        let decoy = format!("{id}!");
        let probe = probe();
        let (_, root) = mounted(
            view,
            vec![
                AgentMessage::ToolResult(result(id, json!("first"))),
                AgentMessage::ToolResult(result(&decoy, json!("decoy"))),
                calling(&[id, &decoy]),
                AgentMessage::ToolResult(result(id, json!("last"))),
            ],
        );
        let seen: Vec<_> = (probe.seen.borrow().iter())
            .map(|seen| json!({"key": seen.id, "details": seen.details}))
            .collect();
        assert_eq!(Value::Array(seen), row["expected"], "{id:?}");
        unmount(&root);
    }
}

/// Unregistered entries: users and attached users show user rows, assistants show their
/// row, and results, unknown roles and artifacts show nothing.
fn fallbacks(view: &DomView) {
    let artifacts = Rc::new(Cell::new(0));
    let reached = Rc::clone(&artifacts);
    role("artifact", move |_| {
        reached.set(reached.get() + 1);
        Ok(Box::new(Label("artifact".into())))
    });
    let (_, root) = mounted(
        view,
        vec![
            artifact(),
            user("ordinary"),
            attached("with file"),
            AgentMessage::ToolResult(result("alone", json!(null))),
            assistant(json!([{"type":"text","text":"answer"}])),
            notice("unregistered", "hidden"),
            artifact(),
        ],
    );
    let shown = rows(&root);
    assert_eq!(shown.len(), 3);
    assert!(text(&shown[0]).contains("ordinary"));
    assert!(text(&shown[1]).contains("with file"));
    assert!(shown[1].query_selector("[data-open]").unwrap().is_some());
    assert!(text(&shown[2]).contains("answer"));
    assert_eq!(
        artifacts.get(),
        0,
        "artifacts are filtered before role renderers"
    );
    unmount(&root);
}

/// A registered role wins for every entry kind and receives the original handle; empty
/// registered content stays selected; a renderer error is returned unchanged.
fn precedence(view: &DomView) {
    let entries = [
        ("user", user("u")),
        ("user-with-attachments", attached("a")),
        ("assistant", assistant(json!([{"type":"text","text":"t"}]))),
        (
            "toolResult",
            AgentMessage::ToolResult(result("r", json!(null))),
        ),
        ("unregistered", notice("unregistered", "n")),
    ];
    for (name, entry) in entries {
        let original = entry.clone();
        let label = format!("custom {name}");
        role(name, move |message| {
            assert!(
                same(message, &original),
                "the renderer receives the original handle"
            );
            Ok(Box::new(Label(label.clone())))
        });
        let (_, root) = mounted(view, vec![entry]);
        assert_eq!(
            rows(&root).iter().map(|row| text(row)).collect::<Vec<_>>(),
            [format!("custom {name}")]
        );
        unmount(&root);
    }
    role("assistant", |_| Ok(Box::new(Label(String::new()))));
    let (_, root) = mounted(view, vec![assistant(json!([{"type":"text","text":"t"}]))]);
    assert_eq!(rows(&root).len(), 1);
    assert_eq!(
        text(&rows(&root)[0]),
        "",
        "empty registered content is not replaced"
    );
    unmount(&root);
    role("unregistered", |_| {
        Err(ToolUiError::Presentation("controlled failure".into()))
    });
    let list = MessageList::<Notice> {
        messages: Arc::new(RwLock::new(vec![notice("unregistered", "n")])),
        ..MessageList::default()
    };
    let error = list.mount(view).err().unwrap();
    assert_eq!(
        error.to_string(),
        ToolUiError::Presentation("controlled failure".into()).to_string()
    );
}

/// Whether two entries share their original handle.
fn same(left: &Entry, right: &Entry) -> bool {
    match (left, right) {
        (AgentMessage::User(left), AgentMessage::User(right)) => Arc::ptr_eq(left, right),
        (AgentMessage::Assistant(left), AgentMessage::Assistant(right)) => Arc::ptr_eq(left, right),
        (AgentMessage::ToolResult(left), AgentMessage::ToolResult(right)) => {
            Arc::ptr_eq(left, right)
        }
        (AgentMessage::Custom(left), AgentMessage::Custom(right)) => Arc::ptr_eq(left, right),
        _ => false,
    }
}

/// Rows keep their roots by emitted position, forward the list's current operands with the
/// stable flags, and the recorded callback edits replay against the live history; an unrelated
/// refresh keeps an unchanged user row's selection, focus and copy feedback.
#[wasm_bindgen]
pub async fn message_list_preserves_emitted_positions_and_current_actions(
    import_module: &js_sys::Function,
) {
    let view = view();
    positions(&view);
    let fixture = fixture();
    forwarding(&view, &fixture["listForwarding"]);
    callbacks(&view, &fixture["listCallbacks"]);
    indexed_result(&view);
    let _elements = crate::contributions_support::host(&view, import_module);
    let page = crate::contributions_support::page_module(import_module).await;
    unchanged_user_row(&view, &page).await;
}

/// An unrelated refresh updates an unchanged user row in place: its selected text, its focused
/// tile and its listing's copy feedback survive with their nodes.
async fn unchanged_user_row(view: &DomView, page: &wasm_bindgen::JsValue) {
    use crate::contributions_support::{Clipboard, Recorded, call, select, selection, until};
    let clipboard = Recorded::install(Clipboard::Accept);
    let (list, root) = mounted(view, vec![attached("selected words\n\n```\ncopied\n```")]);
    let row = rows(&root).remove(0);
    let (words, copy) = (select(&row, "p"), select(&row, "copy-button"));
    select(&copy, "button")
        .unchecked_ref::<HtmlElement>()
        .click();
    until(page, &copy, || text(&copy).contains("Copied!")).await;
    let tile = select(&row, "[data-open]");
    tile.unchecked_ref::<HtmlElement>().focus().unwrap();
    call(page, "select", &js_sys::Array::of1(&words)).await;
    list.messages.write().unwrap().push(user("later"));
    list.update(&root, view).unwrap();
    assert_eq!(rows(&root).len(), 2);
    assert!(select(&row, "p").is_same_node(Some(&words)));
    assert!(select(&row, "copy-button").is_same_node(Some(&copy)));
    assert!(select(&row, "[data-open]").is_same_node(Some(&tile)));
    assert!(
        view.document
            .active_element()
            .unwrap()
            .is_same_node(Some(&tile))
    );
    assert_eq!(
        selection(page, &words).await,
        ("selected words".to_owned(), true)
    );
    assert!(text(&copy).contains("Copied!"));
    assert_eq!(clipboard.texts().len(), 1);
    unmount(&root);
}

/// Filtered entries before or between rows do not move them; a row keeps its root while the
/// entry at its position keeps its kind and is replaced when the kind changes.
fn positions(view: &DomView) {
    let (list, root) = mounted(
        view,
        vec![
            artifact(),
            user("a"),
            artifact(),
            user("a"),
            assistant(json!([{"type":"text","text":"b"}])),
        ],
    );
    let before = rows(&root);
    assert_eq!(before.len(), 3, "duplicate content keeps two rows");
    assert!(!before[0].is_same_node(Some(&before[1])));
    list.messages.write().unwrap().insert(0, artifact());
    list.messages.write().unwrap().remove(3);
    list.update(&root, view).unwrap();
    let after = rows(&root);
    for (old, new) in before.iter().zip(&after) {
        assert!(
            old.is_same_node(Some(new)),
            "filtered entries do not move rows"
        );
    }
    list.messages.write().unwrap().insert(0, user("new"));
    list.update(&root, view).unwrap();
    let shifted = rows(&root);
    assert_eq!(shifted.len(), 4);
    assert!(
        shifted[0].is_same_node(Some(&before[0])),
        "same kind keeps the root"
    );
    assert!(text(&shifted[0]).contains("new"));
    assert!(shifted[1].is_same_node(Some(&before[1])));
    assert!(
        !shifted[2].is_same_node(Some(&before[2])),
        "a changed kind replaces the root"
    );
    assert!(text(&shifted[3]).contains('b'));
    list.messages.write().unwrap().truncate(1);
    list.update(&root, view).unwrap();
    assert_eq!(rows(&root).len(), 1);
    assert!(!before[1].is_connected() && !shifted[2].is_connected());
    unmount(&root);
}

/// Each recorded flag combination: stable assistants never stream, never hide tool calls and
/// hide pending calls only while the list streams; the current cost action is used.
fn forwarding(view: &DomView, records: &Value) {
    assert_eq!(records.as_array().unwrap().len(), 16);
    for row in records.as_array().unwrap() {
        let input = &row["input"];
        let expected = &row["expected"];
        assert_eq!(expected["isStreaming"], false);
        assert_eq!(expected["hideToolCalls"], false);
        let probe = probe();
        let clicks = Rc::new(Cell::new(0));
        let counted = Rc::clone(&clicks);
        let mut list = MessageList::<Notice> {
            messages: Arc::new(RwLock::new(vec![calling(&["waiting"])])),
            is_streaming: input["streaming"].as_bool().unwrap(),
            pending_tool_calls: input["hasPending"]
                .as_bool()
                .unwrap()
                .then(|| Rc::new(HashSet::from(["waiting".to_owned()]))),
            on_cost_click: input["hasCost"]
                .as_bool()
                .unwrap()
                .then(|| Rc::new(move || counted.set(counted.get() + 1)) as Rc<dyn Fn()>),
            ..MessageList::default()
        };
        let root = list.mount(view).unwrap();
        body().append_child(&root.node).unwrap();
        let hidden = expected["hidePendingToolCalls"].as_bool().unwrap()
            && list.pending_tool_calls.is_some();
        let seen = probe.seen.borrow();
        assert_eq!(seen.len(), usize::from(!hidden), "{row}");
        if list.pending_tool_calls.is_none() {
            assert!(seen.iter().all(|seen| !seen.streaming), "{row}");
        }
        drop(seen);
        let usage = rows(&root)[0]
            .query_selector(".maestro-message-usage")
            .unwrap();
        let usage: HtmlElement = usage.expect("stable rows never stream").unchecked_into();
        usage.click();
        assert_eq!(
            clicks.get(),
            usize::from(list.on_cost_click.is_some()),
            "{row}"
        );
        current_action(view, &mut list, &root);
        unmount(&root);
    }
}

/// A replaced cost action is used from the next update; a removed one leaves plain usage.
fn current_action(view: &DomView, list: &mut MessageList<Notice>, root: &DomRoot) {
    let replaced = Rc::new(Cell::new(0));
    let counted = Rc::clone(&replaced);
    list.on_cost_click = Some(Rc::new(move || counted.set(counted.get() + 1)));
    list.update(root, view).unwrap();
    let usage = |root: &DomRoot| -> HtmlElement {
        rows(root)[0]
            .query_selector(".maestro-message-usage")
            .unwrap()
            .unwrap()
            .unchecked_into()
    };
    usage(root).click();
    assert_eq!(replaced.get(), 1);
    list.on_cost_click = None;
    list.update(root, view).unwrap();
    usage(root).click();
    assert_eq!(replaced.get(), 1);
    assert!(!usage(root).class_list().contains("maestro-text-action"));
}

/// The six recorded edits made by a role renderer while the list is prepared.
fn callbacks(view: &DomView, records: &Value) {
    assert_eq!(records.as_array().unwrap().len(), 6);
    for row in records.as_array().unwrap().iter().take(5) {
        let edit = row["edit"].as_str().unwrap().to_owned();
        let first = notice("notice", "first");
        let history = Arc::new(RwLock::new(vec![
            first.clone(),
            user("second"),
            user("third"),
        ]));
        let called = Rc::new(Cell::new(0));
        let (live, count) = (Arc::clone(&history), Rc::clone(&called));
        role("notice", move |message| {
            assert!(same(message, &first));
            count.set(count.get() + 1);
            edited(&edit, message, &live)
        });
        let list = MessageList::<Notice> {
            messages: history,
            ..MessageList::default()
        };
        let expected = &row["expected"];
        match list.mount(view) {
            Ok(root) => {
                let values: Vec<_> = rows(&root)
                    .iter()
                    .map(|row| text(row).trim().to_owned())
                    .collect();
                assert_eq!(json!(values), expected["values"], "{row}");
                assert_eq!(expected["keys"].as_array().unwrap().len(), values.len());
            }
            Err(error) => {
                assert_eq!(expected["throwsOriginal"], true);
                assert_eq!(error.to_string(), "controlled callback failure");
            }
        }
        assert_eq!(json!(called.get()), expected["called"]);
    }
}
/// Apply one recorded edit from inside the role renderer.
fn edited(
    edit: &str,
    message: &Entry,
    history: &Arc<RwLock<Vec<Entry>>>,
) -> Result<Box<dyn DomContent>, ToolUiError> {
    let AgentMessage::Custom(value) = message else {
        unreachable!()
    };
    match edit {
        "mutate" => {
            if let BrowserMessages::Other(notice) = &mut *value.write().unwrap() {
                notice.text = "edited".into();
            }
        }
        "replace" => history.write().unwrap()[1] = user("replacement"),
        "add" => history.write().unwrap().push(user("added")),
        "delete" => drop(history.write().unwrap().remove(1)),
        "throw" => {
            return Err(ToolUiError::Presentation(
                "controlled callback failure".into(),
            ));
        }
        other => panic!("unknown edit {other}"),
    }
    let BrowserMessages::Other(notice) = &*value.read().unwrap() else {
        unreachable!()
    };
    Ok(Box::new(Label(notice.text.clone())))
}

/// A result edited by an earlier renderer keeps its indexed call ID: the assistant row still
/// finds it under the old ID and sees the edited envelope.
fn indexed_result(view: &DomView) {
    let fixture = fixture();
    let expected = &fixture["listCallbacks"][5]["expected"];
    let envelope = result("call", json!({"value": 1}));
    let edited = Arc::clone(&envelope);
    role("notice", move |_| {
        let mut result = edited.write().unwrap();
        result.tool_call_id = "changed".into();
        result.details = Some(json!({"value": 2}));
        Ok(Box::new(Label("notice".into())))
    });
    let probe = probe();
    let (_, root) = mounted(
        view,
        vec![
            notice("notice", ""),
            AgentMessage::ToolResult(envelope),
            calling(&[expected["indexKey"].as_str().unwrap()]),
        ],
    );
    let seen = probe.seen.borrow();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].id.as_deref(), expected["retainedId"].as_str());
    assert_eq!(seen[0].details, Some(json!({"value": expected["details"]})));
    unmount(&root);
}

/// A streaming container mounted under the page body, with the page helpers.
async fn streaming(
    import_module: &js_sys::Function,
) -> (
    Rc<DomView>,
    wasm_bindgen::JsValue,
    StreamingMessageContainer<Notice>,
    DomRoot,
) {
    let view = view();
    let page = crate::contributions_support::page_module(import_module).await;
    let container = StreamingMessageContainer::<Notice>::default();
    let root = container.mount(&view).unwrap();
    body().append_child(&root.node).unwrap();
    (view, page, container, root)
}
/// An assistant entry showing `text`.
fn says(text: &str) -> Entry {
    assistant(json!([{"type":"text","text":text}]))
}
/// Replace the text of an assistant entry in place.
fn edit(entry: &Entry, text: &str) {
    let AgentMessage::Assistant(message) = entry else {
        unreachable!()
    };
    if let Some(maestro_app::AssistantContent::Text(content)) =
        message.write().unwrap().content.first_mut()
    {
        content.text = text.into();
    }
}
/// The streamed assistant text, or all shown text when no assistant text is shown.
fn shown(root: &DomRoot) -> String {
    let node: &Element = root.node.unchecked_ref();
    match node.query_selector(".maestro-markdown").unwrap() {
        Some(markdown) => text(&markdown).trim().to_owned(),
        None => text(node).trim().to_owned(),
    }
}
/// Recorded stream case `id`.
fn case(fixture: &Value, id: &str) -> Value {
    (fixture["stream"].as_array().unwrap().iter())
        .find(|row| row["id"] == id)
        .unwrap()
        .clone()
}

/// Deferred writes publish the latest message on the next frame as a snapshot taken then:
/// edits before the frame show, later edits do not; immediate publication keeps the original
/// message, whose later edits show on the next update; custom payloads need no serialization.
#[wasm_bindgen]
pub async fn streaming_container_snapshots_latest_nested_message(import_module: &js_sys::Function) {
    use crate::contributions_support::frames;
    let fixture = fixture();
    let (view, page, container, root) = streaming(import_module).await;
    let last = case(&fixture, "stream_last_pending");
    for text in last["input"]["texts"].as_array().unwrap() {
        container
            .set_message(Some(says(text.as_str().unwrap())), false)
            .unwrap();
    }
    assert_eq!(shown(&root), "", "nothing publishes before the frame");
    frames(&page, 2).await;
    assert_eq!(shown(&root), last["expected"]["text"]);
    let nested = case(&fixture, "stream_nested_snapshot");
    let entry = says(nested["input"]["initial"].as_str().unwrap());
    container.set_message(Some(entry.clone()), false).unwrap();
    edit(&entry, nested["input"]["beforeFrame"].as_str().unwrap());
    frames(&page, 2).await;
    let published = nested["expected"]["published"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert_eq!(shown(&root), published);
    edit(&entry, nested["input"]["afterFrame"].as_str().unwrap());
    container.update(&root, &view).unwrap();
    assert_eq!(shown(&root), published, "the published snapshot stays");
    let immediate = case(&fixture, "stream_immediate_identity");
    let entry = says("immediate");
    container.set_message(Some(entry.clone()), true).unwrap();
    assert_eq!(
        shown(&root),
        "immediate",
        "immediate publication is synchronous"
    );
    edit(&entry, immediate["expected"]["text"].as_str().unwrap());
    container.update(&root, &view).unwrap();
    assert_eq!(shown(&root), immediate["expected"]["text"]);
    container
        .set_message(Some(notice("notice", "typed payload")), false)
        .unwrap();
    frames(&page, 2).await;
    assert_eq!(
        shown(&root),
        "",
        "a custom entry shows nothing in the stream"
    );
    unmount(&root);
}

/// Clearing is synchronous and retires queued work; the next response publishes on its own
/// first frame; instances and released roots do not settle each other's work.
#[wasm_bindgen]
pub async fn streaming_container_clear_allows_next_response_first_frame(
    import_module: &js_sys::Function,
) {
    use crate::contributions_support::frames;
    let fixture = fixture();
    let (view, page, container, root) = streaming(import_module).await;
    container.set_message(None, false).unwrap();
    container.set_message(Some(says("stale")), false).unwrap();
    container.set_message(None, false).unwrap();
    assert_eq!(shown(&root), "");
    frames(&page, 2).await;
    assert_eq!(shown(&root), "", "retired work cannot publish");
    container
        .set_message(Some(says("published")), false)
        .unwrap();
    frames(&page, 2).await;
    assert_eq!(shown(&root), "published");
    container.set_message(None, false).unwrap();
    assert_eq!(shown(&root), "", "clearing is synchronous");
    container.set_message(None, false).unwrap();
    container
        .set_message(Some(says("replacement")), true)
        .unwrap();
    assert_eq!(shown(&root), "replacement");
    for id in [
        "stream_next_turn_defect_false",
        "stream_next_turn_defect_true",
    ] {
        let row = case(&fixture, id);
        if row["input"]["queued"] == true {
            container.set_message(Some(says("stale")), false).unwrap();
        }
        container.set_message(None, false).unwrap();
        container
            .set_message(Some(says("next turn")), false)
            .unwrap();
        frames(&page, 1).await;
        let required = row["expected"]["required"]["content"][0]["text"]
            .as_str()
            .unwrap();
        assert_eq!(shown(&root), required, "{id}");
    }
    independent(&view, &page, &container, &root).await;
    unmount(&root);
}

/// A second instance publishes on its own while the first is cleared, and a released root is
/// left alone while a remounted one publishes.
async fn independent(
    view: &DomView,
    page: &wasm_bindgen::JsValue,
    first: &StreamingMessageContainer<Notice>,
    root: &DomRoot,
) {
    use crate::contributions_support::frames;
    let second = StreamingMessageContainer::<Notice>::default();
    let other = second.mount(view).unwrap();
    body().append_child(&other.node).unwrap();
    second.set_message(Some(says("second")), false).unwrap();
    first.set_message(None, false).unwrap();
    frames(page, 2).await;
    assert_eq!(shown(&other), "second");
    assert_eq!(shown(root), "");
    unmount(&other);
    second
        .set_message(Some(says("after release")), false)
        .unwrap();
    drop(other);
    frames(page, 2).await;
    let remounted = second.mount(view).unwrap();
    body().append_child(&remounted.node).unwrap();
    assert_eq!(shown(&remounted), "after release");
    second.set_message(Some(says("remounted")), false).unwrap();
    frames(page, 2).await;
    assert_eq!(shown(&remounted), "remounted");
    unmount(&remounted);
}

/// Each recorded branch shows the assistant row, the pulse or nothing; assistant rows receive
/// the container's current operands without hiding tool or pending calls, keep their root
/// through updates and keep a keyboard-usable cost control.
#[wasm_bindgen]
pub async fn streaming_container_branches_keep_real_assistant_controls(
    import_module: &js_sys::Function,
) {
    let fixture = fixture();
    let (view, _page, mut container, root) = streaming(import_module).await;
    assert_eq!(fixture["streamRender"].as_array().unwrap().len(), 12);
    for row in fixture["streamRender"].as_array().unwrap() {
        container.is_streaming = row["input"]["streaming"].as_bool().unwrap();
        let entry = match row["input"]["role"].as_str() {
            None => None,
            Some("user") => Some(user("u")),
            Some("user-with-attachments") => Some(attached("a")),
            Some("toolResult") => Some(AgentMessage::ToolResult(result("r", json!(null)))),
            Some("assistant") => Some(says("streamed")),
            Some(other) => Some(notice(other, "n")),
        };
        container.set_message(entry, true).unwrap();
        let expected = &row["expected"];
        let node: &Element = root.node.unchecked_ref();
        let has = |selector: &str| node.query_selector(selector).unwrap().is_some();
        assert_eq!(
            has(".maestro-assistant-row"),
            expected["hasAssistant"] == true,
            "{row}"
        );
        assert_eq!(
            has(".maestro-stream-pulse"),
            expected["hasPulse"] == true,
            "{row}"
        );
        assert_eq!(
            node.child_element_count() == 0,
            expected["empty"] == true,
            "{row}"
        );
    }
    container.is_streaming = false;
    container.set_message(None, true).unwrap();
    assistant_operands(&view, &mut container, &root).await;
    unmount(&root);
}

/// Current tools, results, pending calls and cost action reach the streamed assistant.
async fn assistant_operands(
    view: &DomView,
    container: &mut StreamingMessageContainer<Notice>,
    root: &DomRoot,
) {
    let probe = probe();
    let clicks = Rc::new(Cell::new(0));
    let counted = Rc::clone(&clicks);
    container.on_cost_click = Some(Rc::new(move || counted.set(counted.get() + 1)));
    container.pending_tool_calls = Some(Rc::new(HashSet::from(["waiting".to_owned()])));
    container.tool_results_by_id = Some(Rc::new(std::collections::HashMap::from([(
        "done".to_owned(),
        result("done", json!("selected")),
    )])));
    container
        .set_message(Some(calling(&["done", "waiting"])), true)
        .unwrap();
    let seen: Vec<_> = probe
        .seen
        .borrow()
        .iter()
        .map(|seen| (seen.id.clone(), seen.details.clone()))
        .collect();
    assert_eq!(
        seen,
        [(Some("done".into()), Some(json!("selected"))), (None, None)],
        "pending calls stay shown"
    );
    let row = root
        .node
        .unchecked_ref::<Element>()
        .query_selector(".maestro-assistant-row")
        .unwrap()
        .unwrap();
    container.update(root, view).unwrap();
    let kept = root
        .node
        .unchecked_ref::<Element>()
        .query_selector(".maestro-assistant-row")
        .unwrap()
        .unwrap();
    assert!(
        kept.is_same_node(Some(&row)),
        "updates keep the assistant root"
    );
    let usage: HtmlElement = row
        .query_selector(".maestro-message-usage")
        .unwrap()
        .unwrap()
        .unchecked_into();
    usage.focus().unwrap();
    crate::contributions_support::driver("driverPress", "Enter").await;
    assert_eq!(clicks.get(), 1, "the cost control is keyboard-usable");
}
