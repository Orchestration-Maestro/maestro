//! The runnable chat example in a real page, driven through its own code and controls.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::{
    contributions_support::{container, focused, host, page_module, select, text, view},
    dialog_support::{Fetch, Prompts, completed_reply, panel, panels, press, until},
};
use maestro_app::{AgentMessage, AppStorage, SessionMetadata};
use maestro_web::dom::DomView;
use std::rc::Rc;
use wasm_bindgen::{JsCast as _, JsValue, prelude::wasm_bindgen};
use web_sys::{Element, HtmlElement, HtmlInputElement, HtmlTextAreaElement};

#[path = "../examples/chat/app.rs"]
mod app;
#[path = "../examples/chat/custom_messages.rs"]
mod custom_messages;

/// The page address without a query, as a fresh visit has it.
fn reset_url() {
    let window = web_sys::window().unwrap();
    let url = web_sys::Url::new(&window.location().href().unwrap()).unwrap();
    url.set_search("");
    window
        .history()
        .unwrap()
        .replace_state_with_url(&JsValue::NULL, "", Some(&url.href()))
        .unwrap();
}
/// The session the address names.
fn url_session() -> Option<String> {
    let window = web_sys::window().unwrap();
    let url = web_sys::Url::new(&window.location().href().unwrap()).unwrap();
    url.search_params().get("session")
}
/// Name the session `id` in the address, as a shared link does.
fn visit(id: &str) {
    let window = web_sys::window().unwrap();
    let url = web_sys::Url::new(&window.location().href().unwrap()).unwrap();
    url.search_params().set("session", id);
    window
        .history()
        .unwrap()
        .replace_state_with_url(&JsValue::NULL, "", Some(&url.href()))
        .unwrap();
}
/// Click the header control titled `title`.
fn click(root: &Element, title: &str) {
    select(root, &format!(".example-header [title='{title}']"))
        .unchecked_into::<HtmlElement>()
        .click();
}
/// The transcript rows.
fn rows(root: &Element) -> u32 {
    root.query_selector_all(".maestro-message-list > *")
        .unwrap()
        .length()
}
/// Type `draft` into the editor and press its Send control.
fn send(root: &Element, draft: &str) {
    let field: HtmlTextAreaElement = select(root, "textarea").unchecked_into();
    field.set_value(draft);
    field
        .dispatch_event(&web_sys::Event::new("input").unwrap())
        .unwrap();
    select(root, ".maestro-editor-send")
        .unchecked_into::<HtmlElement>()
        .click();
}
/// The stored summary of `id` once its save is visible, reading through browser storage.
async fn stored(
    storage: &AppStorage,
    id: &str,
    ready: impl Fn(&SessionMetadata) -> bool,
) -> SessionMetadata {
    loop {
        if let Some(metadata) = storage.sessions.get_metadata(id).await.unwrap()
            && ready(&metadata)
        {
            return metadata;
        }
    }
}
/// Storage with a key for the example's model.
async fn keyed(name: &str) -> Rc<AppStorage> {
    let storage = app::storage(name);
    storage
        .provider_keys
        .set("anthropic", "sk-test")
        .await
        .unwrap();
    storage
}
/// One completed exchange starting with `draft`, waited for in the transcript and in storage.
async fn converse(
    page: &JsValue,
    root: &Element,
    storage: &AppStorage,
    draft: &str,
) -> (String, SessionMetadata) {
    let before = rows(root);
    send(root, draft);
    until(page, || rows(root) == before + 2).await;
    let id = url_session().expect("an exchange names the session in the address");
    let count = f64::from(before + 2);
    let metadata = stored(storage, &id, |metadata| metadata.message_count >= count).await;
    (id, metadata)
}
/// A user message with `text`.
fn user(text: &str) -> AgentMessage<custom_messages::ExampleMessages> {
    serde_json::from_value(serde_json::json!({"role": "user", "content": text, "timestamp": 1}))
        .unwrap()
}

/// Titles come from the first sentence within 50 characters, else the first 50 characters,
/// else 47 and an ellipsis; a session is kept only once it holds an exchange.
#[wasm_bindgen]
pub fn chat_example_titles_and_admission_follow_the_first_user_message() {
    let long = "a".repeat(60);
    for (draft, title) in [
        ("  Plan a trip. Then more", "Plan a trip."),
        (".hidden start", ".hidden start"),
        (
            &format!("{long}. done") as &str,
            &format!("{}...", "a".repeat(47)) as &str,
        ),
        ("Short question?", "Short question?"),
        ("   ", ""),
    ] {
        assert_eq!(app::generate_title(&[user(draft)]), title, "{draft}");
    }
    assert!(!app::should_save_session(&[user("hello")]));
    assert_eq!(app::generate_title(&[]), "");
}

/// Two exchanges: the first titles, names and stores the session; the second keeps the
/// creation time and title.
async fn two_exchanges(page: &JsValue, root: &Element, storage: &AppStorage) -> String {
    assert_eq!(
        text(&select(root, ".example-name")),
        "Maestro Web UI Example"
    );
    assert!(
        url_session().is_none(),
        "nothing is stored before an exchange"
    );
    let (id, first) = converse(page, root, storage, "Plan a trip. Then more").await;
    assert_eq!(
        (first.title.as_str(), first.preview.as_str()),
        ("Plan a trip.", "Plan a trip.")
    );
    assert_eq!((first.usage.input, first.usage.output), (3.0, 1.0));
    assert_eq!(text(&select(root, ".example-title-button")), "Plan a trip.");
    let (same, second) = converse(page, root, storage, "And then?").await;
    assert_eq!(same, id);
    assert_eq!(
        second.created_at, first.created_at,
        "later saves keep the creation time"
    );
    assert_eq!(second.title, "Plan a trip.");
    id
}
/// Reopening the address restores session `id`; a missing session starts anew.
async fn reopen(shown: &Rc<DomView>, root: &Element, storage: Rc<AppStorage>, id: &str) {
    let reopened = app::App::init(shown, Rc::clone(&storage), root)
        .await
        .unwrap();
    assert_eq!(url_session().as_deref(), Some(id));
    assert_eq!(text(&select(root, ".example-title-button")), "Plan a trip.");
    assert_eq!(rows(root), 4, "the restored history is shown");
    drop(reopened);
    visit("missing");
    let fresh = app::App::init(shown, storage, root).await.unwrap();
    assert!(
        url_session().is_none(),
        "a missing session starts a new one"
    );
    assert_eq!(rows(root), 0);
    drop(fresh);
}

/// A first exchange titles the session, names it in the address and stores it; later saves
/// keep its creation time; reopening the address restores it, and a missing one starts anew.
#[wasm_bindgen]
pub async fn chat_example_saves_and_restores_sessions_from_the_url(import: js_sys::Function) {
    let shown = view();
    let _host = host(&shown, &import);
    let page = page_module(&import).await;
    reset_url();
    let storage = keyed("example-save").await;
    let fetch = Fetch::install(vec![
        (200, "text/event-stream", completed_reply()),
        (200, "text/event-stream", completed_reply()),
    ]);
    let root = container();
    let running = app::App::init(&shown, Rc::clone(&storage), &root)
        .await
        .unwrap();
    let id = two_exchanges(&page, &root, &storage).await;
    assert_eq!(fetch.requests().len(), 2);
    drop(running);
    reopen(&shown, &root, storage, &id).await;
}

/// Edit the title of the stored session `id` with the keyboard.
async fn retitle(page: &JsValue, root: &Element, storage: &AppStorage, id: &str) {
    select(root, ".example-title-button")
        .unchecked_into::<HtmlElement>()
        .click();
    let field: HtmlInputElement = select(root, ".example-title input").unchecked_into();
    assert!(focused(&field));
    assert_eq!(field.selection_start().unwrap(), Some(0));
    assert_eq!(
        field.selection_end().unwrap(),
        Some(12),
        "the whole title is selected"
    );
    field.set_value("  Renamed ");
    press("Enter").await;
    stored(storage, id, |metadata| metadata.title == "Renamed").await;
    until(page, || {
        root.query_selector(".example-title-button")
            .unwrap()
            .is_some()
    })
    .await;
    let button = select(root, ".example-title-button");
    assert_eq!(text(&button), "Renamed");
    button.unchecked_ref::<HtmlElement>().click();
    let field: HtmlInputElement = select(root, ".example-title input").unchecked_into();
    field.set_value("Discarded");
    press("Escape").await;
    until(page, || {
        root.query_selector(".example-title-button")
            .unwrap()
            .is_some()
    })
    .await;
    assert_eq!(text(&select(root, ".example-title-button")), "Renamed");
}
/// Choose the stored session `id` from the list, then delete it while it is shown.
async fn choose_and_delete(page: &JsValue, root: &Element, id: &str) {
    click(root, "Sessions");
    let list = panel();
    until(page, || {
        list.query_selector(".maestro-session-row")
            .unwrap()
            .is_some()
    })
    .await;
    select(&list, ".maestro-session-row")
        .unchecked_into::<HtmlElement>()
        .click();
    until(page, || rows(root) == 2).await;
    assert_eq!(url_session().as_deref(), Some(id));
    assert_eq!(text(&select(root, ".example-title-button")), "Renamed");
    let prompts = Prompts::install(vec![true]);
    click(root, "Sessions");
    let list = panel();
    until(page, || {
        list.query_selector(".maestro-session-delete")
            .unwrap()
            .is_some()
    })
    .await;
    select(&list, ".maestro-session-delete")
        .unchecked_into::<HtmlElement>()
        .click();
    until(page, || {
        list.query_selector(".maestro-session-row")
            .unwrap()
            .is_none()
    })
    .await;
    press("Escape").await;
    until(page, || url_session().is_none() && rows(root) == 0).await;
    assert_eq!(prompts.messages().len(), 1);
}

/// The title edits in place with its text selected: Enter stores a new title and Escape keeps
/// the old one. New Session clears the address; choosing a stored session loads it, and
/// deleting the shown session starts a new one.
#[wasm_bindgen]
pub async fn chat_example_edits_titles_and_follows_new_and_deleted_sessions(
    import: js_sys::Function,
) {
    let shown = view();
    let _host = host(&shown, &import);
    let page = page_module(&import).await;
    reset_url();
    let storage = keyed("example-edit").await;
    let _fetch = Fetch::install(vec![(200, "text/event-stream", completed_reply())]);
    let root = container();
    let _running = app::App::init(&shown, Rc::clone(&storage), &root)
        .await
        .unwrap();
    let (id, _) = converse(&page, &root, &storage, "Plan a trip.").await;
    retitle(&page, &root, &storage, &id).await;
    click(&root, "New Session");
    until(&page, || {
        url_session().is_none() && root.query_selector(".example-name").unwrap().is_some()
    })
    .await;
    assert_eq!(rows(&root), 0);
    choose_and_delete(&page, &root, &id).await;
}

/// Sending without a key opens the key prompt; closing it keeps the draft and sends nothing.
async fn refused_key(page: &JsValue, root: &Element, fetch: &Fetch) {
    send(root, "hello");
    until(page, || !panels().is_empty()).await;
    assert!(text(&panel()).contains("API Key Required"));
    press("Escape").await;
    until(page, || panels().is_empty()).await;
    let field: HtmlTextAreaElement = select(root, "textarea").unchecked_into();
    assert_eq!(field.value(), "hello", "a refused key keeps the draft");
    assert!(fetch.requests().is_empty());
}
/// The header holds the page's only appearance control, and Settings shows both tabs.
async fn header_controls(root: &Element) {
    let toggles = root.query_selector_all(".maestro-theme-toggle").unwrap();
    assert_eq!(toggles.length(), 1);
    assert!(select(root, ".example-header").contains(toggles.item(0).as_ref()));
    click(root, "Settings");
    let tabs = text(&panel());
    assert!(
        tabs.contains("Providers & Models") && tabs.contains("Proxy"),
        "{tabs}"
    );
    press("Escape").await;
}

/// Without a key the example asks for one and keeps the draft. The demonstration notice shows
/// as an alert and reaches the model as system text beside both tools; the header keeps its own
/// appearance control while the chat has none, and Settings opens the provider and proxy tabs.
#[wasm_bindgen]
pub async fn chat_example_prompts_notifies_and_opens_settings(import: js_sys::Function) {
    let shown = view();
    let _host = host(&shown, &import);
    let page = page_module(&import).await;
    reset_url();
    let storage = app::storage("example-notify");
    let fetch = Fetch::install(vec![
        (200, "text/event-stream", completed_reply()),
        (200, "text/event-stream", completed_reply()),
    ]);
    let root = container();
    let _running = app::App::init(&shown, Rc::clone(&storage), &root)
        .await
        .unwrap();
    refused_key(&page, &root, &fetch).await;
    storage
        .provider_keys
        .set("anthropic", "sk-test")
        .await
        .unwrap();
    click(&root, "Demo: Add Custom Notification");
    send(&root, "hello");
    until(&page, || {
        root.query_selector("[role=alert]").unwrap().is_some() && rows(&root) >= 3
    })
    .await;
    let notice = text(&select(&root, "[role=alert]"));
    assert!(notice.contains("This is a custom message!"));
    assert!(notice.contains("included in the model context"), "{notice}");
    let body = fetch.requests()[0].body.clone();
    assert!(body.contains("<system>This is a custom message!"));
    assert!(body.contains("\"javascript_repl\"") && body.contains("\"artifacts\""));
    header_controls(&root).await;
}

/// Hold a browser effect through the page's own module import until the test releases it.
async fn held_effect(import: &js_sys::Function, operation: &str, value: &str) -> JsValue {
    let url = wasm_bindgen::link_to!(module = "/tests/chat_example_support.js");
    let module =
        crate::contributions_support::settle(import.call1(&JsValue::NULL, &url.into()).unwrap())
            .await
            .unwrap();
    crate::contributions_support::call(&module, operation, &js_sys::Array::of1(&value.into())).await
}
/// Call a controlled effect's operation.
fn effect(effect: &JsValue, name: &str) {
    js_sys::Reflect::get(effect, &name.into())
        .unwrap()
        .unchecked_into::<js_sys::Function>()
        .call0(effect)
        .unwrap();
}
/// Wait until a controlled effect reaches its hold.
async fn ready(effect: &JsValue) {
    crate::contributions_support::settle(js_sys::Reflect::get(effect, &"ready".into()).unwrap())
        .await
        .unwrap();
}
/// Start a session through the header and wait for its empty transcript and cleared address.
async fn start_new_session(page: &JsValue, root: &Element) {
    click(root, "New Session");
    until(page, || rows(root) == 0 && url_session().is_none()).await;
}
/// Begin editing the title and return its focused field.
fn title_field(root: &Element, draft: &str) -> HtmlInputElement {
    select(root, ".example-title-button")
        .unchecked_into::<HtmlElement>()
        .click();
    let field: HtmlInputElement = select(root, ".example-title input").unchecked_into();
    field.set_value(draft);
    field
}

/// Completing an unrelated message keeps the title draft, node, focus and selection.
#[wasm_bindgen]
pub async fn chat_example_message_completion_keeps_title_drafts(import: js_sys::Function) {
    let shown = view();
    let _host = host(&shown, &import);
    let page = page_module(&import).await;
    reset_url();
    let storage = keyed("example-title-draft").await;
    let _fetch = Fetch::install(vec![(200, "text/event-stream", completed_reply())]);
    let root = container();
    let _running = app::App::init(&shown, Rc::clone(&storage), &root)
        .await
        .unwrap();
    let (id, _) = converse(&page, &root, &storage, "Plan a trip.").await;
    let held = held_effect(&import, "holdResponse", &completed_reply()).await;
    send(&root, "continue");
    ready(&held).await;
    let field = title_field(&root, "Unfinished title");
    field.set_selection_range(2, 7).unwrap();
    effect(&held, "finish");
    until(&page, || rows(&root) == 4).await;
    stored(&storage, &id, |metadata| metadata.message_count >= 4.0).await;
    assert!(field.is_same_node(Some(&select(&root, ".example-title input"))));
    assert_eq!(field.value(), "Unfinished title");
    assert!(focused(&field));
    assert_eq!(
        (
            field.selection_start().unwrap(),
            field.selection_end().unwrap()
        ),
        (Some(2), Some(7))
    );
    effect(&held, "restore");
}

/// A title save from an outgoing session cannot rename the new session or close its editor.
#[wasm_bindgen]
pub async fn chat_example_title_saves_stay_with_the_originating_session(import: js_sys::Function) {
    let shown = view();
    let _host = host(&shown, &import);
    let page = page_module(&import).await;
    reset_url();
    let storage = keyed("example-title-origin").await;
    let _fetch = Fetch::install(vec![
        (200, "text/event-stream", completed_reply()),
        (200, "text/event-stream", completed_reply()),
        (200, "text/event-stream", completed_reply()),
    ]);
    let root = container();
    let _running = app::App::init(&shown, Rc::clone(&storage), &root)
        .await
        .unwrap();
    let (a, _) = converse(&page, &root, &storage, "Session A").await;
    let held = held_effect(&import, "holdTitleWrite", "Renamed A").await;
    title_field(&root, "Renamed A");
    press("Enter").await;
    ready(&held).await;
    start_new_session(&page, &root).await;
    converse(&page, &root, &storage, "Session B").await;
    finish_outgoing_title_while_editing(&page, &root, &held).await;
    let (_, saved) = converse(&page, &root, &storage, "continue B").await;
    assert_eq!(saved.title, "Session B");
    assert_eq!(stored(&storage, &a, |_| true).await.title, "Renamed A");
    effect(&held, "restore");
}

/// Finish the outgoing write while editing the new session, preserving its editor and title.
async fn finish_outgoing_title_while_editing(page: &JsValue, root: &Element, held: &JsValue) {
    let field = title_field(root, "B draft");
    field.set_selection_range(1, 3).unwrap();
    effect(held, "finish");
    crate::contributions_support::frames(page, 2).await;
    assert!(
        field.is_same_node(
            root.query_selector(".example-title input")
                .unwrap()
                .as_deref()
        )
    );
    assert_eq!(field.value(), "B draft");
    assert!(focused(&field));
    press("Escape").await;
    until(page, || {
        root.query_selector(".example-title-button")
            .unwrap()
            .is_some()
    })
    .await;
    assert_eq!(text(&select(root, ".example-title-button")), "Session B");
}

/// New Session signals cancellation to an outgoing controlled streaming request.
#[wasm_bindgen]
pub async fn chat_example_new_session_aborts_the_outgoing_request(import: js_sys::Function) {
    let shown = view();
    let _host = host(&shown, &import);
    let page = page_module(&import).await;
    reset_url();
    let storage = keyed("example-new-abort").await;
    let root = container();
    let _running = app::App::init(&shown, storage, &root).await.unwrap();
    let held = held_effect(&import, "holdResponse", &completed_reply()).await;
    send(&root, "pending");
    ready(&held).await;
    start_new_session(&page, &root).await;
    crate::contributions_support::frames(&page, 2).await;
    assert_eq!(
        js_sys::Reflect::get(&held, &"aborted".into()).unwrap(),
        JsValue::TRUE
    );
    assert_eq!(
        text(&select(&root, ".example-name")),
        "Maestro Web UI Example"
    );
    effect(&held, "restore");
}
