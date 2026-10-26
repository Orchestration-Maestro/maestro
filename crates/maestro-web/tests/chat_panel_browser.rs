//! The chat screen in a real page: tools, restored artifacts, panel actions and the breakpoint.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{computed, container, host, sandbox_url, select, text, view};
use maestro_app::browser::tools::javascript_repl::RuntimeProvidersFactory;
use maestro_app::{AgentInitialState, AgentMessage, AgentOptions, JsonObject, get_model};
use maestro_web::{
    ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RW, ArtifactsPanel, BrowserSandboxHost, BrowserSession,
    ChatPanel, ChatPanelConfig, browser::sandbox_value::BrowserSandboxValue,
    create_javascript_repl_tool, get_tool_renderer,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};
use wasm_bindgen::{JsCast as _, prelude::wasm_bindgen};
use web_sys::{Element, HtmlElement};

/// The browser message family the sessions hold.
type Browser = maestro_app::browser::components::messages::CustomAgentMessages;

/// A session on the controlled model holding `messages`.
fn session(messages: Vec<Value>) -> Rc<BrowserSession> {
    let messages = messages
        .into_iter()
        .map(|message| serde_json::from_value::<AgentMessage<Browser>>(message).unwrap())
        .collect();
    Rc::new(BrowserSession::new_local(AgentOptions {
        initial_state: AgentInitialState {
            model: Some(Arc::new(RwLock::new(
                get_model("anthropic", "claude-haiku-4-5").unwrap(),
            ))),
            messages,
            ..AgentInitialState::default()
        },
        ..AgentOptions::default()
    }))
}
/// A recorded artifact creation.
fn created(filename: &str) -> Value {
    json!({"role": "artifact", "action": "create", "filename": filename,
        "content": "# restored", "timestamp": "2026-01-01T00:00:00.000Z"})
}
/// A user message carrying one text attachment.
fn with_attachment() -> Value {
    json!({"role": "user-with-attachments", "content": "see", "timestamp": 1,
        "attachments": [{"id": "n", "type": "document", "fileName": "n.txt",
            "mimeType": "text/plain", "size": 4, "content": "bm90ZQ==", "extractedText": "note"}]})
}
/// The names of the session's tools in order.
fn tool_names(session: &BrowserSession) -> Vec<String> {
    let tools = Rc::clone(session.state().read().unwrap().tools());
    let tools = tools.read().unwrap();
    tools
        .iter()
        .map(|tool| tool.read().unwrap().definition.name.clone())
        .collect()
}
/// Run the session's artifacts tool with `arguments`.
async fn artifacts(session: &BrowserSession, arguments: Value) {
    let tools = Rc::clone(session.state().read().unwrap().tools());
    let tool = Rc::clone(&tools.read().unwrap()[0]);
    let execute = tool.read().unwrap().execute.clone();
    execute("call".into(), arguments, None, None).await.unwrap();
}
/// Create `filename` through the session's artifacts tool.
async fn create(session: &BrowserSession, filename: &str) {
    artifacts(
        session,
        json!({"command": "create", "filename": filename, "content": "text"}),
    )
    .await;
}
/// Set the reported window width and announce the resize.
fn resize(width: f64) {
    let window = web_sys::window().unwrap();
    js_sys::Reflect::set(&window, &"innerWidth".into(), &width.into()).unwrap();
    window
        .dispatch_event(&web_sys::Event::new("resize").unwrap())
        .unwrap();
}
/// The chat panel mounted in a fixed-height section.
fn mounted(chat: &ChatPanel) -> Element {
    let parent = container();
    parent
        .set_attribute("style", "height:600px;display:flex;flex-direction:column")
        .unwrap();
    parent.append_child(chat.root()).unwrap();
    parent
}
/// The side placement of the artifacts panel.
fn side(chat: &ChatPanel) -> Element {
    select(chat.root(), ".maestro-chat-side")
}
/// The column holding the session view.
fn main_column(chat: &ChatPanel) -> HtmlElement {
    select(chat.root(), ".maestro-chat-main").unchecked_into()
}
/// The floating artifact count control.
fn pill(chat: &ChatPanel) -> HtmlElement {
    select(chat.root(), ".maestro-chat-pill").unchecked_into()
}

/// What the tools factory received: the view's root, the panel and the runtime providers.
type Received = (
    Element,
    Rc<ArtifactsPanel>,
    RuntimeProvidersFactory<BrowserSandboxValue>,
);
/// Operands whose factory checks it receives `expected`, records what else it received and adds
/// a JavaScript tool reading those runtime providers.
fn recording(
    expected: &Rc<BrowserSession>,
    received: &Rc<RefCell<Option<Received>>>,
) -> ChatPanelConfig {
    let seen = Rc::clone(received);
    let expected = Rc::clone(expected);
    ChatPanelConfig {
        sandbox_url_provider: Some(sandbox_url()),
        tools_factory: Some(Rc::new(move |agent, interface, panel, providers| {
            assert!(Rc::ptr_eq(agent, &expected));
            let repl = create_javascript_repl_tool(Arc::new(BrowserSandboxHost));
            *repl.runtime_providers_factory.write().unwrap() = Some(Rc::clone(&providers));
            *seen.borrow_mut() = Some((interface.root().clone(), Rc::clone(panel), providers));
            vec![repl.local_tool()]
        })),
        ..ChatPanelConfig::default()
    }
}
/// The session's tools, the factory operands and the restored, still closed panel.
fn assert_composed(chat: &ChatPanel, first: &Rc<BrowserSession>, received: &Received) {
    let (interface, panel, providers) = received;
    assert_eq!(tool_names(first), ["artifacts", "javascript_repl"]);
    let tools = Rc::clone(first.state().read().unwrap().tools());
    let describe = tools.read().unwrap()[1]
        .read()
        .unwrap()
        .description
        .clone()
        .unwrap();
    assert!(
        describe()
            .unwrap()
            .contains(ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RW)
    );
    assert!(chat.agent().is_some_and(|agent| Rc::ptr_eq(&agent, first)));
    assert!(interface.is_same_node(Some(chat.agent_interface().unwrap().root())));
    assert!(chat.root().contains(Some(interface)));
    assert!(Rc::ptr_eq(panel, &chat.artifacts_panel().unwrap()));
    assert!(
        interface
            .query_selector(".maestro-theme-toggle")
            .unwrap()
            .is_none()
    );
    assert!(get_tool_renderer::<JsonObject, Value>("artifacts").is_none());
    let restored = providers().unwrap();
    assert_eq!(restored.len(), 2, "attachments and artifacts");
    let data = restored[1].get_data().unwrap();
    assert!(format!("{data:?}").contains("notes.md"));
    assert!(
        side(chat).has_attribute("hidden"),
        "restored panel stays closed"
    );
    assert!(!pill(chat).has_attribute("hidden"));
    assert_eq!(count(chat), "1");
}

/// Without a session the panel says so; a session gets the artifacts tool before the caller's
/// tools, the factory receives the original session, view and panel, and its runtime providers
/// read current attachments and artifacts. History restores artifacts without opening the panel.
#[wasm_bindgen]
pub async fn chat_panel_agent_tools_runtime_factory_and_restore(import: js_sys::Function) {
    let shown = view();
    let _host = host(&shown, &import);
    resize(1280.0);
    let chat = ChatPanel::new(&shown).unwrap();
    mounted(&chat);
    assert_eq!(text(chat.root()), "No agent set");
    let first = session(vec![created("notes.md"), with_attachment()]);
    let received = Rc::new(RefCell::new(None));
    chat.set_agent(Rc::clone(&first), recording(&first, &received))
        .await
        .unwrap();
    let received = received.take().unwrap();
    assert_composed(&chat, &first, &received);
    let second = session(Vec::new());
    chat.set_agent(Rc::clone(&second), ChatPanelConfig::default())
        .await
        .unwrap();
    assert!(
        !chat.root().contains(Some(&received.0)),
        "the old view is replaced"
    );
    assert_eq!(tool_names(&second), ["artifacts"]);
    assert!(pill(&chat).has_attribute("hidden"));
    assert_eq!(
        (received.2)().unwrap().len(),
        2,
        "earlier tools keep their own session and panel"
    );
}

/// Click the open panel's Close control.
fn close(chat: &ChatPanel) {
    select(&side(chat), "[data-close-artifacts]")
        .unchecked_into::<HtmlElement>()
        .click();
}
/// The inline width of the chat column.
fn main_width(chat: &ChatPanel) -> String {
    main_column(chat)
        .style()
        .get_property_value("width")
        .unwrap()
}
/// The count the closed panel's control shows.
fn count(chat: &ChatPanel) -> String {
    text(&select(&pill(chat), ".maestro-chat-pill-count"))
}
/// Whether the panel is shown.
fn open(chat: &ChatPanel) -> bool {
    !side(chat).has_attribute("hidden")
}
/// A new artifact opens the panel and Close closes it; another instance sees none of it.
async fn open_and_close(chat: &ChatPanel, other: &ChatPanel, first: &BrowserSession) {
    assert!(!open(chat));
    assert!(pill(chat).has_attribute("hidden"));
    assert_eq!(main_width(chat), "100%");
    create(first, "a.txt").await;
    assert!(open(chat), "a new artifact opens the panel");
    assert_eq!(main_width(chat), "50%");
    assert!(
        pill(other).has_attribute("hidden"),
        "other instances are unaffected"
    );
    close(chat);
    assert!(!open(chat));
    assert_eq!(main_width(chat), "100%");
    assert_eq!(count(chat), "1");
    assert_eq!(pill(chat).title(), "Show artifacts");
}
/// An edit keeps the closed panel closed, another new artifact opens it and the count control
/// reopens it.
async fn edit_create_and_reopen(chat: &ChatPanel, first: &BrowserSession) {
    artifacts(
        first,
        json!({"command": "rewrite", "filename": "a.txt", "content": "again"}),
    )
    .await;
    assert!(!open(chat), "an edit keeps it closed");
    create(first, "b.txt").await;
    assert!(open(chat));
    close(chat);
    assert_eq!(count(chat), "2");
    pill(chat).click();
    assert!(open(chat), "the count control reopens it");
    assert!(pill(chat).has_attribute("hidden"));
}
/// Whether the panel is placed over the chat.
fn overlaid(chat: &ChatPanel) -> bool {
    side(chat).class_list().contains("maestro-dialog-backdrop")
}

/// A new artifact opens the panel and an edit does not; close and the count control move it
/// out of and back into view; the row splits at 800 px and overlays below; instances are
/// independent and a detached panel stops following the window.
#[wasm_bindgen]
pub async fn chat_panel_artifact_counts_breakpoint_and_native_actions(import: js_sys::Function) {
    let shown = view();
    let _host = host(&shown, &import);
    resize(1280.0);
    let chat = ChatPanel::new(&shown).unwrap();
    let parent = mounted(&chat);
    let other = ChatPanel::new(&shown).unwrap();
    mounted(&other);
    let first = session(Vec::new());
    chat.set_agent(Rc::clone(&first), ChatPanelConfig::default())
        .await
        .unwrap();
    other
        .set_agent(session(Vec::new()), ChatPanelConfig::default())
        .await
        .unwrap();
    open_and_close(&chat, &other, &first).await;
    edit_create_and_reopen(&chat, &first).await;
    for (width, overlay) in [(799.0, true), (800.0, false), (801.0, false), (799.0, true)] {
        resize(width);
        assert_eq!(overlaid(&chat), overlay);
        assert_eq!(main_width(&chat), if overlay { "100%" } else { "50%" });
    }
    chat.root().remove();
    resize(1280.0);
    assert!(overlaid(&chat), "a detached panel ignores the window");
    parent.append_child(chat.root()).unwrap();
    assert!(!overlaid(&chat));
}

/// The topmost element at the center of `element`.
fn hit_center(element: &Element) -> Element {
    use num_traits::ToPrimitive as _;
    let bounds = element.get_bounding_client_rect();
    let x = (bounds.left() + bounds.width() / 2.0).to_f32().unwrap();
    let y = (bounds.top() + bounds.height() / 2.0).to_f32().unwrap();
    crate::contributions_support::document()
        .element_from_point(x, y)
        .unwrap()
}
/// The sharp overlay fills the viewport and keeps its controls reachable under `presentation`.
fn assert_overlay(chat: &ChatPanel, presentation: &maestro_theme::BrandPresentation<'_>) {
    let backdrop = side(chat);
    let panel = select(&backdrop, ".maestro-artifacts-panel");
    let style = chat.root().unchecked_ref::<HtmlElement>().style();
    for (key, value) in presentation.css_properties() {
        style.set_property(&key, &value).unwrap();
    }
    assert_eq!(computed(&backdrop, "position"), "fixed");
    assert_eq!(computed(&backdrop, "backdrop-filter"), "blur(4px)");
    assert_eq!(
        computed(&backdrop, "background-color"),
        "rgba(0, 0, 0, 0.5)"
    );
    assert_eq!(computed(&panel, "backdrop-filter"), "none");
    assert_eq!(
        computed(&panel, "background-color"),
        crate::contributions_support::rgb(presentation.colors["background"])
    );
    let corner = crate::contributions_support::document()
        .element_from_point(2.0, 2.0)
        .unwrap();
    assert!(
        panel.contains(Some(&corner)),
        "the artifact overlay fills the viewport"
    );
    let bounds = panel.get_bounding_client_rect();
    let window = web_sys::window().unwrap();
    assert_eq!((bounds.left(), bounds.top()), (0.0, 0.0));
    assert!(
        (bounds.width()
            - f64::from(
                window
                    .document()
                    .unwrap()
                    .document_element()
                    .unwrap()
                    .client_width()
            ))
        .abs()
            < 1.0
    );
    assert!((bounds.height() - window.inner_height().unwrap().as_f64().unwrap()).abs() < 1.0);
    assert_eq!(computed(&panel, "border-radius"), "0px");
    assert_eq!(computed(&panel, "border-left-width"), "0px");
    let tab = select(&panel, ".maestro-artifact-tabs button");
    assert!(
        tab.contains(Some(&hit_center(&tab))),
        "the panel's controls stay reachable"
    );
}

/// Below the breakpoint the panel fills the viewport above the backdrop, without an inset,
/// border or rounded corners, in both packs and modes.
#[wasm_bindgen]
pub async fn chat_panel_overlay_dims_and_blurs_behind_a_sharp_panel(import: js_sys::Function) {
    let shown = view();
    let _host = host(&shown, &import);
    resize(799.0);
    let chat = ChatPanel::new(&shown).unwrap();
    mounted(&chat);
    let first = session(Vec::new());
    chat.set_agent(Rc::clone(&first), ChatPanelConfig::default())
        .await
        .unwrap();
    create(&first, "a.txt").await;
    create(&first, "b.txt").await;
    for name in ["brand.json", "signal.json"] {
        let pack = crate::brand::pack(name).unwrap();
        for mode in [
            maestro_theme::BrandMode::Dark,
            maestro_theme::BrandMode::Light,
        ] {
            assert_overlay(&chat, &pack.presentation(mode).unwrap());
        }
    }
    let tab: HtmlElement = select(&side(&chat), ".maestro-artifact-tabs button").unchecked_into();
    tab.click();
    assert_eq!(tab.get_attribute("aria-pressed").as_deref(), Some("true"));
}

/// Standalone chat mounts native artifact bodies without a contribution host; registering the
/// host afterwards and creating another chat reuse the same built-in constructors.
#[wasm_bindgen]
pub async fn chat_panel_registers_builtins_without_a_contribution_host(import: js_sys::Function) {
    let shown = view();
    let chat = ChatPanel::new(&shown).unwrap();
    mounted(&chat);
    let first = session(Vec::new());
    chat.set_agent(Rc::clone(&first), ChatPanelConfig::default())
        .await
        .unwrap();
    create(&first, "notes.txt").await;
    assert_eq!(
        text(&select(
            chat.root(),
            "text-artifact .maestro-text-artifact code"
        )),
        "text"
    );
    artifacts(
        &first,
        json!({"command": "create", "filename": "notes.md", "content": "# Visible"}),
    )
    .await;
    assert_eq!(
        text(&select(chat.root(), "markdown-artifact h1")),
        "Visible"
    );
    assert!(
        chat.root()
            .query_selector("copy-button button")
            .unwrap()
            .is_some()
    );
    let registry = web_sys::window().unwrap().custom_elements();
    let original = registry.get("text-artifact");
    let _host = host(&shown, &import);
    let _second = ChatPanel::<maestro_app::NoCustomMessages>::new(&shown).unwrap();
    assert!(js_sys::Object::is(
        &original,
        &registry.get("text-artifact")
    ));
}

/// Artifact filename pills rendered after another chat is initialized still open their own
/// panel; replacing that other chat's session cannot replace the first chat's renderer.
#[wasm_bindgen]
pub async fn chat_panel_filename_pills_stay_with_their_own_chat(import: js_sys::Function) {
    let shown = view();
    let _host = host(&shown, &import);
    let a = ChatPanel::new(&shown).unwrap();
    let b = ChatPanel::new(&shown).unwrap();
    mounted(&a);
    mounted(&b);
    let first = session(Vec::new());
    a.set_agent(Rc::clone(&first), ChatPanelConfig::default())
        .await
        .unwrap();
    create(&first, "shared.txt").await;
    close(&a);
    for _ in 0..2 {
        let second = session(Vec::new());
        b.set_agent(Rc::clone(&second), ChatPanelConfig::default())
            .await
            .unwrap();
        create(&second, "shared.txt").await;
        close(&b);
        let call = json!({"role": "assistant", "api": "anthropic-messages", "provider": "anthropic", "model": "claude-haiku-4-5",
            "content": [{"type": "toolCall", "id": "file", "name": "artifacts", "arguments": {"command": "get", "filename": "shared.txt"}}],
            "usage": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "totalTokens": 0, "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0}},
            "stopReason": "toolUse", "timestamp": 1});
        first
            .state()
            .write()
            .unwrap()
            .set_messages(vec![serde_json::from_value(call).unwrap()]);
        let interface = a.agent_interface().unwrap();
        interface.root().remove();
        main_column(&a).append_child(interface.root()).unwrap();
        select(a.root(), "button.maestro-artifact-pill")
            .unchecked_into::<HtmlElement>()
            .click();
        assert!(open(&a), "the originating chat opens");
        assert!(!open(&b), "another chat stays closed");
        close(&a);
    }
}
