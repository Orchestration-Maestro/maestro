//! The session view in a real page: request defaults, send admission, history and reader-follow.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::{
    contributions_support::{container, frames, host, page_module, text, view},
    dialog_support::{Fetch, completed_reply, press, settled, storage},
};
use maestro_app::{
    AgentInitialState, AgentMessage, AgentOptions, CustomProvider, DiagnosticErrorInfo, Model,
    Presence, get_model,
};
use maestro_web::{
    AgentInterface, AgentInterfaceProps, Attachment, BrowserSession, SharedAttachment,
};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};
use wasm_bindgen::{JsCast as _, prelude::wasm_bindgen};
/// The browser message family the sessions hold.
type Browser = maestro_app::browser::components::messages::CustomAgentMessages;
use web_sys::{Element, HtmlElement};

/// A session on `model` with `messages` and no request defaults.
fn session(model: Model, messages: Vec<AgentMessage<Browser>>) -> Rc<BrowserSession> {
    Rc::new(BrowserSession::new_local(AgentOptions {
        initial_state: AgentInitialState {
            model: Some(Arc::new(RwLock::new(model))),
            messages,
            ..AgentInitialState::default()
        },
        ..AgentOptions::default()
    }))
}
/// The controlled Anthropic model.
fn haiku() -> Model {
    get_model("anthropic", "claude-haiku-4-5").unwrap()
}
/// A manual provider's model served at `http://manual.test`.
fn local_model() -> Model {
    serde_json::from_value(serde_json::json!({
        "id": "local-model", "name": "local-model", "api": "openai-completions",
        "provider": "Local Box", "baseUrl": "http://manual.test", "reasoning": false,
        "input": ["text"], "contextWindow": 32000, "maxTokens": 2048,
        "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0}
    }))
    .unwrap()
}
/// A stored manual provider named `Local Box` with `key`.
fn local_provider(key: Option<&str>) -> CustomProvider {
    CustomProvider {
        id: "box".into(),
        name: "Local Box".into(),
        r#type: serde_json::from_value("openai-completions".into()).unwrap(),
        base_url: "http://manual.test".into(),
        api_key: key.map_or(Presence::Missing, |key| Presence::Present(key.into())),
        models: Presence::Present(vec![local_model()]),
    }
}
/// One completed chat-completions reply saying `ok`.
fn completion_reply() -> String {
    [
        r#"{"id":"c","object":"chat.completion.chunk","created":1,"model":"local-model","choices":[{"index":0,"delta":{"role":"assistant","content":"ok"},"finish_reason":null}]}"#,
        r#"{"id":"c","object":"chat.completion.chunk","created":1,"model":"local-model","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":3,"completion_tokens":1,"total_tokens":4}}"#,
        "[DONE]",
    ]
    .iter()
    .fold(String::new(), |mut stream, data| {
        stream.push_str("data: ");
        stream.push_str(data);
        stream.push_str("\n\n");
        stream
    })
}
/// A small text attachment named `name`.
fn note(name: &str) -> SharedAttachment {
    let attachment: Attachment = serde_json::from_value(serde_json::json!({
        "id": name, "type": "document", "fileName": name, "mimeType": "text/plain",
        "size": 4, "content": "bm90ZQ==", "extractedText": "note"
    }))
    .unwrap();
    Arc::new(RwLock::new(attachment))
}
/// Providers asked for a key, answered from `answers` in order.
type Asked = Rc<RefCell<Vec<String>>>;
/// Operands for `session` recording key requests answered from `answers`.
fn props(session: &Rc<BrowserSession>, asked: &Asked, answers: Vec<bool>) -> AgentInterfaceProps {
    let asked = Rc::clone(asked);
    let answers = Rc::new(RefCell::new(answers.into_iter()));
    AgentInterfaceProps {
        session: Some(Rc::clone(session)),
        on_api_key_required: Some(Rc::new(move |provider| {
            asked.borrow_mut().push(provider);
            let answer = answers.borrow_mut().next().unwrap_or(false);
            Box::pin(std::future::ready(answer))
        })),
        ..AgentInterfaceProps::default()
    }
}
/// The view mounted in a section `height` pixels tall.
fn mounted(props: AgentInterfaceProps, height: u32) -> (AgentInterface, Element) {
    let parent = container();
    parent
        .set_attribute(
            "style",
            &format!("height:{height}px;display:flex;flex-direction:column"),
        )
        .unwrap();
    let interface = AgentInterface::new(&view(), props).unwrap();
    parent.append_child(interface.root()).unwrap();
    (interface, parent)
}
/// The kinds of the session's messages.
fn roles(session: &BrowserSession) -> Vec<&'static str> {
    let state = session.state().read().unwrap();
    let messages = state.messages().read().unwrap();
    messages
        .iter()
        .map(|message| match message {
            AgentMessage::User(_) => "user",
            AgentMessage::Assistant(_) => "assistant",
            AgentMessage::ToolResult(_) => "toolResult",
            AgentMessage::Custom(custom) => match &*custom.read().unwrap() {
                Browser::UserWithAttachments(user) => {
                    assert_eq!(user.attachments.as_ref().map(Vec::len), Some(1));
                    "user-with-attachments"
                }
                _ => "custom",
            },
        })
        .collect()
}

/// A blank draft does nothing; a missing key asks and a refusal keeps the draft; a failed
/// pre-send hook keeps it too; an admitted send clears it once, keeps the attachments and moves
/// the finished response into history without duplicating it.
#[wasm_bindgen]
pub async fn agent_interface_send_hooks_keep_session_and_attachments(import: js_sys::Function) {
    let shown = view();
    let _host = host(&shown, &import);
    let app = storage("agent-send");
    let fetch = Fetch::install(vec![(200, "text/event-stream", completed_reply())]);
    let session = session(haiku(), Vec::new());
    let asked: Asked = Rc::default();
    let (interface, _parent) = mounted(props(&session, &asked, vec![false]), 400);
    interface.send_message(" \n", Vec::new()).await.unwrap();
    interface.set_input("hello", vec![note("a.txt")]).unwrap();
    interface
        .send_message("hello", vec![note("a.txt")])
        .await
        .unwrap();
    assert_eq!(*asked.borrow(), ["anthropic"], "a missing key asks once");
    assert_eq!(
        interface.editor().value(),
        "hello",
        "a refusal keeps the draft"
    );
    app.provider_keys.set("anthropic", "sk-test").await.unwrap();
    let failing = AgentInterfaceProps {
        on_before_send: Some(Rc::new(|| {
            Box::pin(std::future::ready(Err(DiagnosticErrorInfo::error(
                "not now",
            ))))
        })),
        ..props(&session, &asked, vec![])
    };
    interface.update(failing).unwrap();
    assert!(
        interface
            .send_message("hello", vec![note("a.txt")])
            .await
            .is_err()
    );
    assert_eq!(
        (
            interface.editor().value(),
            interface.editor().attachments().len()
        ),
        ("hello".to_owned(), 1),
        "a failed hook keeps the draft"
    );
    assert!(fetch.requests().is_empty());
    let hooked = Rc::new(RefCell::new(0));
    let count = Rc::clone(&hooked);
    interface
        .update(AgentInterfaceProps {
            on_before_send: Some(Rc::new(move || {
                *count.borrow_mut() += 1;
                Box::pin(std::future::ready(Ok(())))
            })),
            ..props(&session, &asked, vec![])
        })
        .unwrap();
    let attachments = interface.editor().attachments();
    interface.send_message("hello", attachments).await.unwrap();
    admitted(&interface, &session, &fetch, &hooked);
}
/// The admitted send cleared the draft once and the finished response sits in history once.
fn admitted(
    interface: &AgentInterface,
    session: &BrowserSession,
    fetch: &Fetch,
    hooked: &Rc<RefCell<u32>>,
) {
    assert_eq!(*hooked.borrow(), 1);
    assert_eq!(fetch.requests().len(), 1);
    assert_eq!(
        (
            interface.editor().value(),
            interface.editor().attachments().len()
        ),
        (String::new(), 0)
    );
    assert_eq!(roles(session), ["user-with-attachments", "assistant"]);
    let root = interface.root();
    let rows = root
        .query_selector_all(".maestro-message-list > *")
        .unwrap();
    assert_eq!(rows.length(), 2, "history shows each message once");
    let stream = root.query_selector(".maestro-stream").unwrap().unwrap();
    assert!(stream.parent_element().unwrap().has_attribute("hidden"));
    assert_eq!(
        text(&stream).trim(),
        "",
        "the finished response left the stream"
    );
}

/// A custom provider's stored key sends without asking; a keyless custom provider asks.
#[wasm_bindgen]
pub async fn agent_interface_custom_provider_keys_follow_storage(import: js_sys::Function) {
    let shown = view();
    let _host = host(&shown, &import);
    let app = storage("agent-custom");
    app.custom_providers
        .set(&local_provider(Some("sk-local")))
        .await
        .unwrap();
    let fetch = Fetch::install(vec![(200, "text/event-stream", completion_reply())]);
    let session = session(local_model(), Vec::new());
    let asked: Asked = Rc::default();
    let (interface, _parent) = mounted(props(&session, &asked, vec![false]), 400);
    interface.send_message("hi", Vec::new()).await.unwrap();
    assert!(
        asked.borrow().is_empty(),
        "a stored custom key needs no prompt"
    );
    let requests = fetch.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url, "http://manual.test/chat/completions");
    assert!(
        requests[0]
            .headers
            .iter()
            .any(|(name, value)| name == "authorization" && value == "Bearer sk-local")
    );
    app.custom_providers
        .set(&local_provider(None))
        .await
        .unwrap();
    interface.set_input("again", Vec::new()).unwrap();
    interface.send_message("again", Vec::new()).await.unwrap();
    assert_eq!(
        *asked.borrow(),
        ["Local Box"],
        "a keyless custom provider asks"
    );
    assert_eq!(interface.editor().value(), "again");
    assert_eq!(fetch.requests().len(), 1);
}

/// Empty stored keys ask before sending, and do not hide a custom provider's nonempty key.
#[wasm_bindgen]
pub async fn agent_interface_empty_keys_prompt_without_sending(import: js_sys::Function) {
    let shown = view();
    let _host = host(&shown, &import);
    let app = storage("agent-empty-keys");
    app.custom_providers
        .set(&local_provider(Some("")))
        .await
        .unwrap();
    let fetch = Fetch::install(vec![]);
    for model in [haiku(), local_model()] {
        let provider = model.provider.clone();
        app.provider_keys.set(&provider, "").await.unwrap();
        let session = session(model, Vec::new());
        let asked: Asked = Rc::default();
        let (interface, _parent) = mounted(props(&session, &asked, vec![false]), 400);
        let lookup = session.get_api_key.read().unwrap().clone().unwrap();
        assert_eq!(lookup(provider.clone()).await.unwrap(), None);
        interface.set_input("keep me", Vec::new()).unwrap();
        interface.send_message("keep me", Vec::new()).await.unwrap();
        assert_eq!(*asked.borrow(), [provider]);
        assert_eq!(interface.editor().value(), "keep me");
        assert!(fetch.requests().is_empty());
    }
    app.custom_providers
        .set(&local_provider(Some("sk-local")))
        .await
        .unwrap();
    let session = session(local_model(), Vec::new());
    let (_interface, _parent) = mounted(props(&session, &Rc::default(), vec![]), 400);
    let lookup = session.get_api_key.read().unwrap().clone().unwrap();
    assert_eq!(
        lookup("Local Box".into()).await.unwrap().as_deref(),
        Some("sk-local")
    );
}

/// Without a session the view says so; a session gets the browser's spawner, proxy-aware stream
/// and stored-key lookup only where unset; replacing it moves the subscription.
#[wasm_bindgen]
pub async fn agent_interface_session_replacement_and_request_configuration(
    import: js_sys::Function,
) {
    let shown = view();
    let _host = host(&shown, &import);
    let app = storage("agent-session");
    let (interface, _parent) = mounted(AgentInterfaceProps::default(), 400);
    assert_eq!(text(interface.root()), "No session set");
    let first = session(haiku(), Vec::new());
    let asked: Asked = Rc::default();
    interface.update(props(&first, &asked, vec![])).unwrap();
    assert!(first.spawn.read().unwrap().is_some());
    assert!(first.stream_fn.read().unwrap().is_some());
    let lookup = first.get_api_key.read().unwrap().clone().unwrap();
    assert_eq!(lookup("anthropic".into()).await.unwrap(), None);
    app.provider_keys
        .set("anthropic", "sk-ant-oat-1")
        .await
        .unwrap();
    app.custom_providers
        .set(&local_provider(Some("sk-local")))
        .await
        .unwrap();
    assert_eq!(
        lookup("anthropic".into()).await.unwrap().as_deref(),
        Some("sk-ant-oat-1")
    );
    assert_eq!(
        lookup("Local Box".into()).await.unwrap().as_deref(),
        Some("sk-local")
    );
    let second = session(haiku(), Vec::new());
    let supplied = first.stream_fn.read().unwrap().clone().unwrap();
    *second.stream_fn.write().unwrap() = Some(Rc::clone(&supplied));
    interface.update(props(&second, &asked, vec![])).unwrap();
    let kept = second.stream_fn.read().unwrap().clone().unwrap();
    assert!(Rc::ptr_eq(&kept, &supplied), "a supplied stream is kept");
    assert!(second.get_api_key.read().unwrap().is_some());
    proxied(&interface, &first).await;
}
/// The first session's run no longer reaches the view; the proxy setting is read per request.
async fn proxied(interface: &AgentInterface, first: &Rc<BrowserSession>) {
    let app = maestro_app::get_app_storage().unwrap();
    app.settings.set("proxy.enabled", &true).await.unwrap();
    app.settings
        .set("proxy.url", &"https://proxy.test")
        .await
        .unwrap();
    let fetch = Fetch::install(vec![
        (200, "text/event-stream", completed_reply()),
        (200, "text/event-stream", completed_reply()),
    ]);
    first
        .prompt(maestro_app::PromptInput::Text {
            text: "elsewhere".into(),
            images: Vec::new(),
        })
        .await
        .unwrap();
    settled().await;
    let rows = interface
        .root()
        .query_selector_all(".maestro-message-list > *")
        .unwrap();
    assert_eq!(rows.length(), 0, "a replaced session no longer renders");
    interface.send_message("here", Vec::new()).await.unwrap();
    let rows = interface
        .root()
        .query_selector_all(".maestro-message-list > *")
        .unwrap();
    assert_eq!(rows.length(), 2);
    let urls: Vec<_> = fetch
        .requests()
        .into_iter()
        .map(|request| request.url)
        .collect();
    assert!(
        urls.iter()
            .all(|url| url.starts_with("https://proxy.test/?url=")),
        "{urls:?}"
    );
}

/// A user entry saying `text`.
fn said(text: &str) -> AgentMessage<Browser> {
    serde_json::from_value(serde_json::json!({"role": "user", "content": text, "timestamp": 1}))
        .unwrap()
}
/// An assistant entry with usage.
fn answered() -> AgentMessage<Browser> {
    serde_json::from_value(serde_json::json!({
        "role": "assistant", "content": [{"type": "text", "text": "answer"}],
        "api": "anthropic-messages", "provider": "anthropic", "model": "claude-haiku-4-5",
        "usage": {"input": 10, "output": 5, "cacheRead": 0, "cacheWrite": 0, "totalTokens": 15,
            "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0.005}},
        "stopReason": "stop", "timestamp": 2
    }))
    .unwrap()
}
/// The transcript scroller.
fn scroller(interface: &AgentInterface) -> HtmlElement {
    interface
        .root()
        .query_selector(".maestro-agent-scroll")
        .unwrap()
        .unwrap()
        .unchecked_into()
}
/// Scroll to `distance` pixels from the bottom and let the scroll event arrive.
async fn scroll_to(page: &wasm_bindgen::JsValue, scroller: &HtmlElement, distance: f64) {
    let bottom = f64::from(scroller.scroll_height() - scroller.client_height());
    js_sys::Reflect::set(scroller, &"scrollTop".into(), &(bottom - distance).into()).unwrap();
    frames(page, 2).await;
}
/// Grow the history by one entry and let the resize observation arrive.
async fn grow(
    page: &wasm_bindgen::JsValue,
    interface: &AgentInterface,
    session: &Rc<BrowserSession>,
) {
    session
        .state()
        .read()
        .unwrap()
        .messages()
        .write()
        .unwrap()
        .push(said("more"));
    interface
        .update(props(session, &Rc::default(), vec![]))
        .unwrap();
    frames(page, 3).await;
}
/// The scroller's distance from the bottom.
fn distance(scroller: &HtmlElement) -> f64 {
    let top = js_sys::Reflect::get(scroller, &"scrollTop".into())
        .unwrap()
        .as_f64()
        .unwrap();
    f64::from(scroller.scroll_height() - scroller.client_height()) - top
}

/// New content follows only while the reader stays near the bottom: more than 50 px up stops
/// following, 10 px or more keeps it stopped, a fractional distance under 10 px resumes it, and
/// the usage totals are an action reachable from the keyboard.
#[wasm_bindgen]
pub async fn agent_interface_scroll_equality_relayout_and_cost_actions(import: js_sys::Function) {
    let shown = view();
    let _host = host(&shown, &import);
    let page = page_module(&import).await;
    let _app = storage("agent-scroll");
    let mut messages: Vec<_> = (0..30)
        .map(|index| said(&format!("line {index}")))
        .collect();
    messages.push(answered());
    let session = session(haiku(), messages);
    let clicks = Rc::new(RefCell::new(0));
    let count = Rc::clone(&clicks);
    let (interface, _parent) = mounted(
        AgentInterfaceProps {
            on_cost_click: Some(Rc::new(move || *count.borrow_mut() += 1)),
            ..props(&session, &Rc::default(), vec![])
        },
        300,
    );
    frames(&page, 3).await;
    cost_action(&interface, &clicks).await;
    let scroller = scroller(&interface);
    assert!(distance(&scroller) < 1.0, "the view starts at the bottom");
    scroll_to(&page, &scroller, 60.0).await;
    grow(&page, &interface, &session).await;
    assert!(distance(&scroller) > 50.0, "reading up stops following");
    scroll_to(&page, &scroller, 10.4).await;
    let held = distance(&scroller);
    grow(&page, &interface, &session).await;
    assert!(
        held >= 10.0 && distance(&scroller) > 10.0,
        "{held} px does not resume"
    );
    scroll_to(&page, &scroller, 9.2).await;
    let near = distance(&scroller);
    grow(&page, &interface, &session).await;
    assert!(
        near < 10.0 && distance(&scroller) < 1.0,
        "{near} px follows again"
    );
}
/// The usage totals are a button: a click and the keyboard both call the action.
async fn cost_action(interface: &AgentInterface, clicks: &Rc<RefCell<u32>>) {
    let usage = interface
        .root()
        .query_selector("button.maestro-agent-usage")
        .unwrap()
        .unwrap();
    assert_eq!(text(&usage), "↑10 ↓5 $0.0050");
    usage.unchecked_ref::<HtmlElement>().click();
    usage.unchecked_ref::<HtmlElement>().focus().unwrap();
    press("Enter").await;
    assert_eq!(*clicks.borrow(), 2);
}

/// A pending pre-send hook never clears a newer text or attachment draft, even after an undo.
#[wasm_bindgen]
pub async fn agent_interface_pending_admission_preserves_newer_drafts(import: js_sys::Function) {
    let shown = view();
    let _host = host(&shown, &import);
    let app = storage("agent-pending");
    app.provider_keys.set("anthropic", "sk-test").await.unwrap();
    let fetch = Fetch::install(
        (0..3)
            .map(|_| (200, "text/event-stream", completed_reply()))
            .collect(),
    );
    let session = session(haiku(), Vec::new());
    let (interface, _parent) = mounted(props(&session, &Rc::default(), vec![]), 400);
    let interface = Rc::new(interface);
    for draft in ["new text", "hello", "undo"] {
        let (release, send) = pending_send(&interface, &session).await;
        interface.set_input(draft, vec![note("new.txt")]).unwrap();
        if draft == "undo" {
            interface.set_input("hello", vec![note("old.txt")]).unwrap();
        }
        let expected = interface.editor().value();
        let expected_attachment = interface.editor().attachments()[0]
            .read()
            .unwrap()
            .file_name
            .clone();
        release.send(()).unwrap();
        wasm_bindgen_futures::JsFuture::from(send).await.unwrap();
        assert_eq!(
            interface.editor().value(),
            expected,
            "a newer draft survives admission"
        );
        assert_eq!(
            interface.editor().attachments()[0]
                .read()
                .unwrap()
                .file_name,
            expected_attachment
        );
    }
    assert_eq!(fetch.requests().len(), 3);
}

/// Removing a retained view releases session rendering and reattachment binds it once.
#[wasm_bindgen]
pub async fn agent_interface_detach_reconnect_keeps_draft_and_subscription(
    import: js_sys::Function,
) {
    let shown = view();
    let _host = host(&shown, &import);
    let page = page_module(&import).await;
    let app = storage("agent-detach");
    app.provider_keys.set("anthropic", "sk-test").await.unwrap();
    let _fetch = Fetch::install(vec![(200, "text/event-stream", completed_reply())]);
    let session = session(haiku(), Vec::new());
    let (interface, parent) = mounted(props(&session, &Rc::default(), vec![]), 400);
    interface
        .set_input("keep draft", vec![note("keep.txt")])
        .unwrap();
    frames(&page, 2).await;
    interface.root().remove();
    session
        .prompt(maestro_app::PromptInput::Text {
            text: "elsewhere".into(),
            images: Vec::new(),
        })
        .await
        .unwrap();
    let rows = || {
        interface
            .root()
            .query_selector_all(".maestro-message-list > *")
            .unwrap()
            .length()
    };
    assert_eq!(
        rows(),
        0,
        "a detached view no longer renders session events"
    );
    for _ in 0..2 {
        parent.append_child(interface.root()).unwrap();
        frames(&page, 2).await;
        assert_eq!(rows(), 2, "reconnect refreshes the transcript");
        assert_eq!(interface.editor().value(), "keep draft");
        assert_eq!(interface.editor().attachments().len(), 1);
        interface.root().remove();
    }
    reconnect_during_pending_tool(&page).await;
}

/// Completing a detached response clears its partial before reconnecting during tool execution.
async fn reconnect_during_pending_tool(page: &wasm_bindgen::JsValue) {
    let reply = completed_reply().replace(
        concat!("event: message_delta\ndata: ", r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":1}}"#),
        concat!(
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"held\",\"name\":\"held_tool\",\"input\":{}}}\n\n",
            "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":1}\n\n",
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"output_tokens\":1}}"
        ),
    );
    let _fetch = Fetch::install(vec![
        (200, "text/event-stream", reply),
        (200, "text/event-stream", completed_reply()),
    ]);
    let session = session(haiku(), Vec::new());
    let (interface, parent) = mounted(props(&session, &Rc::default(), vec![]), 400);
    let (partial, finish_message, _updates) = hold_event(&session, |event| {
        matches!(event, maestro_app::AgentEvent::MessageUpdate { message, .. }
            if message.read().unwrap().content.iter().any(|content|
                matches!(content, maestro_app::AssistantContent::Text(text) if text.text == "ok")))
    });
    let (tool, finish_tool, _tools) = hold_event(&session, |event| {
        matches!(event, maestro_app::AgentEvent::ToolExecutionStart { .. })
    });
    let run = session.prompt(maestro_app::PromptInput::Text {
        text: "hold the tool".into(),
        images: Vec::new(),
    });
    partial.await.unwrap();
    frames(page, 2).await;
    let stream = interface
        .root()
        .query_selector(".maestro-stream")
        .unwrap()
        .unwrap();
    assert!(text(&stream).contains("ok"), "the partial is published");
    interface.root().remove();
    finish_message.send(()).unwrap();
    tool.await.unwrap();
    {
        let state = session.state().read().unwrap();
        assert!(state.is_streaming());
        assert!(state.streaming_message().is_none());
        assert_eq!(state.pending_tool_calls().as_ref(), ["held"]);
    }
    parent.append_child(interface.root()).unwrap();
    assert_eq!(roles(&session), ["user", "assistant"]);
    assert!(!stream.parent_element().unwrap().has_attribute("hidden"));
    assert!(
        text(&stream).trim().is_empty(),
        "reconnect clears the completed partial while a tool is pending"
    );
    finish_tool.send(()).unwrap();
    run.await.unwrap();
}

/// Pause the first matching event after the view has observed it, exposing entry and release.
fn hold_event(
    session: &Rc<BrowserSession>,
    matches: fn(&maestro_app::AgentEvent<Browser>) -> bool,
) -> (
    tokio::sync::oneshot::Receiver<()>,
    tokio::sync::oneshot::Sender<()>,
    maestro_app::Subscription<Browser, maestro_app::Local>,
) {
    let (entered, entering) = tokio::sync::oneshot::channel();
    let (release, pending) = tokio::sync::oneshot::channel();
    let gate = Rc::new(RefCell::new(Some((entered, pending))));
    let subscription = session.subscribe(Rc::new(move |event, _| {
        let gate = matches(&event.read().unwrap())
            .then(|| gate.borrow_mut().take())
            .flatten();
        Box::pin(async move {
            if let Some((entered, pending)) = gate {
                entered.send(()).unwrap();
                pending.await.unwrap();
            }
            Ok(())
        })
    }));
    (entering, release, subscription)
}

/// Hold one send at its pre-send hook, returning its release and completion.
async fn pending_send(
    interface: &Rc<AgentInterface>,
    session: &Rc<BrowserSession>,
) -> (tokio::sync::oneshot::Sender<()>, js_sys::Promise) {
    let (release, pending) = tokio::sync::oneshot::channel();
    let (entered, entering) = tokio::sync::oneshot::channel();
    let entered = Rc::new(RefCell::new(Some(entered)));
    let pending = Rc::new(RefCell::new(Some(pending)));
    interface
        .update(AgentInterfaceProps {
            on_before_send: Some(Rc::new(move || {
                let pending = pending.borrow_mut().take().unwrap();
                entered.borrow_mut().take().unwrap().send(()).unwrap();
                Box::pin(async move {
                    pending.await.unwrap();
                    Ok(())
                })
            })),
            ..props(session, &Rc::default(), vec![])
        })
        .unwrap();
    interface.set_input("hello", vec![note("old.txt")]).unwrap();
    let sending = Rc::clone(interface);
    let send = wasm_bindgen_futures::future_to_promise(async move {
        sending
            .send_message("hello", sending.editor().attachments())
            .await
            .unwrap();
        Ok(wasm_bindgen::JsValue::UNDEFINED)
    });
    entering.await.unwrap();
    (release, send)
}
