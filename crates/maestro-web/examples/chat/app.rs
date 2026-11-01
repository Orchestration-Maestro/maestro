//! The embedding: browser stores, the chat screen, sessions saved per message, a header with
//! session, notification, appearance and settings controls, and session restore from the URL.
use super::custom_messages::{
    ExampleMessages, SystemNotificationMessage, Variant, create_system_notification,
    custom_convert_to_llm, register_custom_message_renderers,
};
use maestro_app::{
    AgentEvent, AgentInitialState, AgentMessage, AgentOptions, AppStorage, CustomProvidersStore,
    IndexedDBConfig, Local, ProviderKeysStore, SessionData, SessionMetadata, SessionsStore,
    SettingsStore, StorageBackend, Subscription, ThinkingLevel, get_model, set_app_storage,
};
use maestro_web::{
    ApiKeyPromptDialog, BrowserSandboxHost, BrowserSession, ChatPanel, ChatPanelConfig,
    IndexedDBStorageBackend, ProvidersModelsTab, ProxyTab, SessionListDialog, SettingsDialog,
    ThemeToggle,
    components::agent_interface::usage_totals,
    components::button::{ButtonProps, ButtonSize, ButtonVariant, button},
    components::input::Input,
    create_javascript_repl_tool,
    dom::DomView,
    icons::{IconSize, icon_dom, icons},
};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, PoisonError, RwLock},
};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use web_sys::{Element, Event};

mod title;
pub use title::{generate_title, should_save_session};

/// The example's sessions.
pub type Session = BrowserSession<SystemNotificationMessage>;

/// The instructions every session of the example starts with.
const SYSTEM_PROMPT: &str = "You are a helpful AI assistant with access to various tools.

Available tools:
- JavaScript REPL: Execute JavaScript code in a sandboxed browser environment (can do calculations, get time, process data, create visualizations, etc.)
- Artifacts: Create interactive HTML, SVG, Markdown, and text artifacts

Feel free to use these tools when needed to provide accurate and helpful responses.";
/// The header text before a session has a title.
const NAME: &str = "Maestro Web UI Example";
/// The demonstration notification's text.
const NOTICE: &str =
    "This is a custom message! It appears in the UI and is included in the model context.";

/// Application storage over the browser database `name`, published as the current storage.
#[must_use]
pub fn storage(name: &str) -> Rc<AppStorage> {
    let settings = Rc::new(SettingsStore::default());
    let provider_keys = Rc::new(ProviderKeysStore::default());
    let sessions = Rc::new(SessionsStore::default());
    let custom_providers = Rc::new(CustomProvidersStore::default());
    let backend: Rc<dyn StorageBackend> = Rc::new(IndexedDBStorageBackend::new(IndexedDBConfig {
        db_name: name.into(),
        version: 1.0,
        stores: vec![
            settings.get_config(),
            SessionsStore::get_metadata_config(),
            provider_keys.get_config(),
            custom_providers.get_config(),
            sessions.get_config(),
        ],
    }));
    for store in [
        &**settings,
        &**provider_keys,
        &**sessions,
        &**custom_providers,
    ] {
        store.set_backend(Rc::clone(&backend));
    }
    let storage = Rc::new(AppStorage::new(
        settings,
        provider_keys,
        sessions,
        custom_providers,
        backend,
    ));
    set_app_storage(Rc::clone(&storage));
    storage
}

/// Show `session_id` in the address without a navigation; `None` clears the query.
fn update_url(session_id: Option<&str>) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("missing window")?;
    let url = web_sys::Url::new(&window.location().href()?)?;
    match session_id {
        Some(id) => url.search_params().set("session", id),
        None => url.set_search(""),
    }
    window
        .history()?
        .replace_state_with_url(&JsValue::NULL, "", Some(&url.href()))
}
/// The session the address names.
fn url_session() -> Option<String> {
    let window = web_sys::window()?;
    let url = web_sys::Url::new(&window.location().href().ok()?).ok()?;
    url.search_params().get("session")
}
/// The current time as an ISO 8601 timestamp.
fn now() -> String {
    js_sys::Date::new_0().to_iso_string().into()
}
/// A failure as a native value, for the console.
fn js(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}
/// Log a failure that has no visible outcome.
fn report(result: Result<(), JsValue>) {
    if let Err(error) = result {
        web_sys::console::error_1(&error);
    }
}
/// A `tag` element carrying `class`.
fn element(view: &DomView, tag: &str, class: &str) -> Result<Element, JsValue> {
    let element = view.document.create_element(tag)?;
    element.set_class_name(class);
    Ok(element)
}

/// The shown session and its stored identity.
#[derive(Default)]
struct Current {
    /// The stored identity, once the session has been saved.
    session_id: Option<String>,
    /// The title, once one was generated, loaded or edited.
    title: String,
    /// When the stored session was first saved.
    created_at: Option<String>,
    /// Whether the title is being edited.
    editing_title: bool,
    /// The shown session.
    agent: Option<Rc<Session>>,
    /// The session's change subscription.
    subscription: Option<Subscription<ExampleMessages, Local>>,
}
/// One retained native listener.
type Listener = Closure<dyn Fn(Event)>;
/// The running example; dropping it ends its listeners.
pub struct App {
    /// State the callbacks reach weakly.
    state: Rc<State>,
}
/// Page operands, stores, the chat screen and the header.
struct State {
    /// Page operands.
    view: Rc<DomView>,
    /// The stores.
    storage: Rc<AppStorage>,
    /// The page's application element.
    root: Element,
    /// The chat screen.
    chat: ChatPanel<SystemNotificationMessage>,
    /// The header row.
    header: Element,
    /// Holds the title, its button or its editor.
    title: Element,
    /// The header's appearance control.
    theme: ThemeToggle,
    /// The title editor while editing.
    title_input: RefCell<Option<Input>>,
    /// The shown session.
    current: RefCell<Current>,
    /// Header control listeners.
    listeners: RefCell<Vec<Listener>>,
    /// The title button's listener.
    title_listener: RefCell<Option<Listener>>,
}

impl App {
    /// Start the example in `root`: restore the session the address names, or start a new one.
    /// # Errors
    /// Returns a localization or native failure.
    pub async fn init(
        view: &Rc<DomView>,
        storage: Rc<AppStorage>,
        root: &Element,
    ) -> Result<Self, JsValue> {
        register_custom_message_renderers();
        let loading = element(view, "div", "example-loading")?;
        loading.set_text_content(Some("Loading..."));
        root.replace_children_with_node_1(&loading);
        let state = Rc::new(State::build(view, storage, root)?);
        state.listen()?;
        let restored = match url_session() {
            Some(id) => state.load_session(&id).await?,
            None => false,
        };
        if !restored {
            state.new_session().await?;
        }
        root.replace_children_with_node_2(&state.header, state.chat.root());
        Ok(Self { state })
    }
}
impl Drop for App {
    fn drop(&mut self) {
        if let Some(subscription) = self.state.current.borrow_mut().subscription.take() {
            subscription.unsubscribe();
        }
        self.state.root.replace_children_with_node_0();
    }
}
/// A ghost header button titled `title` showing the catalog glyph `glyph`.
fn header_button(
    view: &DomView,
    glyph: &str,
    title: &str,
) -> Result<web_sys::HtmlButtonElement, JsValue> {
    let node = icons()
        .map_err(js)?
        .swap_remove(glyph)
        .ok_or_else(|| js(format!("missing icon {glyph}")))?;
    let icon = icon_dom(&node, IconSize::Sm, None)?;
    let props = ButtonProps {
        variant: ButtonVariant::Ghost,
        size: ButtonSize::Sm,
        title: Some(title),
        ..ButtonProps::default()
    };
    button(&view.document, &props, &[icon.into()])
}
impl State {
    /// Build the header and the chat screen.
    fn build(view: &Rc<DomView>, storage: Rc<AppStorage>, root: &Element) -> Result<Self, JsValue> {
        let header = element(view, "div", "example-header")?;
        let left = element(view, "div", "example-header-start")?;
        let right = element(view, "div", "example-header-end")?;
        let title = element(view, "div", "example-title")?;
        for (glyph, label) in [("History", "Sessions"), ("Plus", "New Session")] {
            let control = header_button(view, glyph, label)?;
            left.append_child(&control)?;
        }
        left.append_child(&title)?;
        let theme = ThemeToggle::new(view, false).map_err(js)?;
        let notify = header_button(view, "Bell", "Demo: Add Custom Notification")?;
        let settings = header_button(view, "Settings", "Settings")?;
        right.append_child(&notify)?;
        right.append_child(theme.root())?;
        right.append_child(&settings)?;
        header.append_child(&left)?;
        header.append_child(&right)?;
        Ok(Self {
            view: Rc::clone(view),
            storage,
            root: root.clone(),
            chat: ChatPanel::new(view).map_err(js)?,
            header,
            title,
            theme,
            title_input: RefCell::default(),
            current: RefCell::default(),
            listeners: RefCell::default(),
            title_listener: RefCell::default(),
        })
    }
    /// Route the header buttons, in their order, to their actions.
    fn listen(self: &Rc<Self>) -> Result<(), JsValue> {
        let actions: [fn(&Rc<Self>); 4] = [
            Self::open_sessions,
            Self::start_new,
            Self::notify,
            Self::open_settings,
        ];
        let buttons = self.header.query_selector_all("button[title]")?;
        let buttons = (0..buttons.length()).filter_map(|index| buttons.item(index));
        let buttons = buttons.filter(|button| !self.theme.root().contains(Some(button)));
        for (button, action) in buttons.zip(actions) {
            let listener =
                Closure::<dyn Fn(Event)>::new(self.weakly(move |state, _| action(state)));
            button.add_event_listener_with_callback("click", listener.as_ref().unchecked_ref())?;
            self.listeners.borrow_mut().push(listener);
        }
        Ok(())
    }
    /// A callback running `action` while the state exists.
    pub(super) fn weakly<A, F: Fn(&Rc<Self>, A) + 'static>(
        self: &Rc<Self>,
        action: F,
    ) -> impl Fn(A) + use<A, F> {
        let weak = Rc::downgrade(self);
        move |argument| {
            if let Some(state) = weak.upgrade() {
                action(&state, argument);
            }
        }
    }
    /// Run `work` on the page's task queue, logging its failure.
    pub(super) fn spawn<F: std::future::Future<Output = Result<(), JsValue>> + 'static>(
        self: &Rc<Self>,
        work: impl FnOnce(Rc<Self>) -> F,
    ) {
        wasm_bindgen_futures::spawn_local(report_async(work(Rc::clone(self))));
    }
    /// Start a new session on the page's task queue.
    fn start_new(self: &Rc<Self>) {
        self.spawn(|state| async move { state.new_session().await });
    }
    /// Load the stored session `id` on the page's task queue.
    fn load(self: &Rc<Self>, id: String) {
        self.spawn(|state| async move { state.load_session(&id).await.map(drop) });
    }
    /// Start a new session when the shown one was deleted.
    fn deleted(self: &Rc<Self>, id: &str) {
        if self.current.borrow().session_id.as_deref() == Some(id) {
            self.start_new();
        }
    }
    /// Open the stored sessions: a choice loads it, and deleting the shown one starts anew.
    fn open_sessions(self: &Rc<Self>) {
        let select = Rc::new(self.weakly(|state, id: String| state.load(id)));
        let delete = Rc::new(self.weakly(|state, id: String| state.deleted(&id)));
        report(
            SessionListDialog::open(&self.view, select, Some(delete))
                .map(drop)
                .map_err(js),
        );
    }
    /// Open the provider, model and proxy settings.
    fn open_settings(self: &Rc<Self>) {
        let tabs: [Rc<dyn maestro_web::SettingsTab>; 2] =
            [Rc::new(ProvidersModelsTab), Rc::new(ProxyTab)];
        report(
            SettingsDialog::open(&self.view, &tabs, None)
                .map(drop)
                .map_err(js),
        );
    }
    /// Queue the demonstration notification for the session's next turn.
    fn notify(self: &Rc<Self>) {
        let agent = self.current.borrow().agent.clone();
        if let Some(agent) = agent {
            let notice = create_system_notification(NOTICE, Variant::Default);
            agent.steer(AgentMessage::Custom(Arc::new(RwLock::new(
                ExampleMessages::Other(notice),
            ))));
        }
    }
}
/// Await `work`, logging its failure.
async fn report_async(work: impl std::future::Future<Output = Result<(), JsValue>>) {
    report(work.await);
}
impl State {
    /// Show a new session with the example's instructions and default model.
    async fn new_session(self: &Rc<Self>) -> Result<(), JsValue> {
        update_url(None)?;
        let model = get_model("anthropic", "claude-sonnet-4-5-20250929").ok_or("missing model")?;
        {
            let mut current = self.current.borrow_mut();
            current.session_id = None;
            current.title.clear();
            current.created_at = None;
            current.editing_title = false;
        }
        self.create_agent(AgentInitialState {
            model: Some(Arc::new(RwLock::new(model))),
            thinking_level: Some(ThinkingLevel::Off),
            ..AgentInitialState::default()
        })
        .await
    }
    /// Show the stored session `id`; `false` when it does not exist.
    async fn load_session(self: &Rc<Self>, id: &str) -> Result<bool, JsValue> {
        let sessions = &self.storage.sessions;
        let Some(data) = sessions.get::<ExampleMessages>(id).await.map_err(js)? else {
            web_sys::console::error_1(&format!("Session not found: {id}").into());
            return Ok(false);
        };
        let metadata = sessions.get_metadata(id).await.map_err(js)?;
        {
            let mut current = self.current.borrow_mut();
            current.session_id = Some(id.into());
            current.title = metadata.map(|metadata| metadata.title).unwrap_or_default();
            current.created_at = Some(data.created_at.clone());
            current.editing_title = false;
        }
        let messages = data
            .messages
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        self.create_agent(AgentInitialState {
            model: Some(data.model),
            thinking_level: Some(data.thinking_level),
            messages,
            ..AgentInitialState::default()
        })
        .await?;
        update_url(Some(id))?;
        Ok(true)
    }
    /// Replace the shown session with one starting from `initial`, saving it as it changes.
    async fn create_agent(
        self: &Rc<Self>,
        initial: AgentInitialState<ExampleMessages, Local>,
    ) -> Result<(), JsValue> {
        let outgoing = self.current.borrow().agent.clone();
        if let Some(outgoing) = outgoing {
            outgoing.abort();
        }
        if let Some(subscription) = self.current.borrow_mut().subscription.take() {
            subscription.unsubscribe();
        }
        let agent = Rc::new(Session::new_local(AgentOptions {
            initial_state: AgentInitialState {
                system_prompt: Some(SYSTEM_PROMPT.into()),
                ..initial
            },
            convert_to_llm: Some(Rc::new(|messages: Arc<RwLock<Vec<_>>>| {
                let messages = messages.read().unwrap_or_else(PoisonError::into_inner);
                let converted = custom_convert_to_llm(&messages);
                Box::pin(std::future::ready(Ok(converted)))
            })),
            ..AgentOptions::default()
        }));
        let weak = Rc::downgrade(self);
        let listener: maestro_app::AgentListener<ExampleMessages, Local> =
            Rc::new(move |event, _| {
                let ended = matches!(
                    &*event.read().unwrap_or_else(PoisonError::into_inner),
                    AgentEvent::MessageEnd { .. }
                );
                if let Some(state) = weak.upgrade().filter(|_| ended) {
                    state.changed();
                }
                Box::pin(std::future::ready(Ok(())))
            });
        let subscription = agent.subscribe(listener);
        {
            let mut current = self.current.borrow_mut();
            current.agent = Some(Rc::clone(&agent));
            current.subscription = Some(subscription);
        }
        self.render_title()?;
        self.chat
            .set_agent(agent, self.chat_config())
            .await
            .map_err(js)
    }
    /// Ask for missing keys and add a JavaScript tool reading the chat's runtime providers.
    fn chat_config(&self) -> ChatPanelConfig<SystemNotificationMessage> {
        let view = Rc::clone(&self.view);
        ChatPanelConfig {
            on_api_key_required: Some(Rc::new(move |provider: String| {
                match ApiKeyPromptDialog::prompt(&view, &provider) {
                    Ok(stored) => Box::pin(stored),
                    Err(error) => {
                        web_sys::console::error_1(&js(error));
                        Box::pin(std::future::ready(false))
                    }
                }
            })),
            tools_factory: Some(Rc::new(|_, _, _, runtime_providers_factory| {
                let repl = create_javascript_repl_tool(Arc::new(BrowserSandboxHost));
                *repl
                    .runtime_providers_factory
                    .write()
                    .unwrap_or_else(PoisonError::into_inner) = Some(runtime_providers_factory);
                vec![repl.local_tool()]
            })),
            ..ChatPanelConfig::default()
        }
    }
    /// After a completed message: title the session, give it an identity once it holds an
    /// exchange, then save it.
    fn changed(self: &Rc<Self>) {
        let Some(agent) = self.current.borrow().agent.clone() else {
            return;
        };
        let messages = Arc::clone(
            agent
                .state()
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .messages(),
        );
        let messages = messages
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let save = should_save_session(&messages);
        let mut current = self.current.borrow_mut();
        if current.title.is_empty() && save {
            current.title = generate_title(&messages);
        }
        if current.session_id.is_none() && save {
            let id = web_sys::window()
                .and_then(|window| window.crypto().ok())
                .map(|crypto| crypto.random_uuid());
            current.created_at = Some(now());
            current.session_id = id;
            report(update_url(current.session_id.as_deref()));
        }
        let stored = current.session_id.is_some();
        drop(current);
        if stored {
            self.spawn(|state| async move { state.save_session().await });
        }
        report(self.render_title());
    }
    /// Store the shown session once it has an identity, a title and an exchange.
    async fn save_session(self: &Rc<Self>) -> Result<(), JsValue> {
        let (id, title, created_at, agent) = {
            let current = self.current.borrow();
            match (&current.session_id, &current.agent) {
                (Some(id), Some(agent)) if !current.title.is_empty() => (
                    id.clone(),
                    current.title.clone(),
                    current.created_at.clone().unwrap_or_else(now),
                    Rc::clone(agent),
                ),
                _ => return Ok(()),
            }
        };
        let (model, thinking_level, messages) = {
            let state = agent.state().read().unwrap_or_else(PoisonError::into_inner);
            (
                Arc::clone(&state.model),
                state.thinking_level.clone(),
                Arc::clone(state.messages()),
            )
        };
        let snapshot = messages
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if !should_save_session(&snapshot) {
            return Ok(());
        }
        let last_modified = now();
        let metadata = SessionMetadata {
            id: id.clone(),
            title: title.clone(),
            created_at: created_at.clone(),
            last_modified: last_modified.clone(),
            message_count: f64::from(u32::try_from(snapshot.len()).unwrap_or(u32::MAX)),
            usage: usage_totals(&snapshot).unwrap_or_default(),
            thinking_level: thinking_level.clone(),
            preview: generate_title(&snapshot),
        };
        let data = SessionData {
            id,
            title,
            model,
            thinking_level,
            messages,
            created_at,
            last_modified,
        };
        self.storage
            .sessions
            .save(&data, &metadata)
            .await
            .map_err(js)
    }
}
