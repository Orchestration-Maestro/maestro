//! The chat screen: a session view beside the artifacts its tools create.
use crate::{
    AgentInterface, AgentInterfaceProps, ApiKeyRequired, ArtifactsPanel, ArtifactsToolRenderer,
    BeforeSend, BrowserSession,
    browser::{
        attachment_content::browser::{append, element, host_error, set_text_if_changed},
        event_subscription::EventSubscription,
        sandbox_value::BrowserSandboxValue,
    },
    components::{connection, message_editor::EditorControls},
    dom::{DomView, MountedContent, report},
    tools::types::{ToolRendererBinding, ToolUiError},
};
use maestro_app::{
    CustomAgentMessages, JsonObject, Local, NoCustomMessages, SandboxRuntimeProvider,
    SandboxUrlProvider, SharedAgentTool,
    browser::{
        components::sandbox::{
            artifacts_runtime_provider::{AgentLike, ArtifactsRuntimeProvider},
            attachments_runtime_provider::AttachmentsRuntimeProvider,
        },
        tools::{
            artifacts::{ArtifactsParams, LocalArtifactsPanel},
            javascript_repl::RuntimeProvidersFactory,
        },
    },
};
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    sync::{Arc, PoisonError, RwLock},
};
use wasm_bindgen::{JsCast as _, closure::Closure};
use web_sys::{Element, Event, HtmlElement};

/// Window width in CSS pixels below which the artifacts panel overlays the chat.
const BREAKPOINT: f64 = 800.0;

/// Builds a chat's additional tools from its session, session view, artifacts panel and the
/// runtime providers a JavaScript tool should offer.
pub type ToolsFactory<C> = Rc<
    dyn Fn(
        &Rc<BrowserSession<C>>,
        &Rc<AgentInterface<C>>,
        &Rc<ArtifactsPanel>,
        RuntimeProvidersFactory<BrowserSandboxValue>,
    ) -> Vec<SharedAgentTool<Local>>,
>;

/// Operands of [`ChatPanel::set_agent`]; every callback is optional.
pub struct ChatPanelConfig<C: CustomAgentMessages + 'static = NoCustomMessages> {
    /// Ask for a missing provider key before sending.
    pub on_api_key_required: Option<ApiKeyRequired>,
    /// Run before each admitted send.
    pub on_before_send: Option<BeforeSend>,
    /// Make the usage totals an action.
    pub on_cost_click: Option<Rc<dyn Fn()>>,
    /// Replace the built-in model picker.
    pub on_model_select: Option<Rc<dyn Fn()>>,
    /// The address HTML artifact frames load.
    pub sandbox_url_provider: Option<SandboxUrlProvider>,
    /// Add tools after the artifacts tool.
    pub tools_factory: Option<ToolsFactory<C>>,
}
impl<C: CustomAgentMessages + 'static> Default for ChatPanelConfig<C> {
    fn default() -> Self {
        Self {
            on_api_key_required: None,
            on_before_send: None,
            on_cost_click: None,
            on_model_select: None,
            sandbox_url_provider: None,
            tools_factory: None,
        }
    }
}

/// One retained native listener.
type Listener = EventSubscription<Closure<dyn Fn(Event)>>;

/// The chat screen; dropping it releases its session view, panel and listeners.
pub struct ChatPanel<C: CustomAgentMessages + 'static = NoCustomMessages> {
    /// Shared state the listeners reach weakly.
    state: Rc<Chat<C>>,
}
/// Nodes, operands and panel state retained across updates.
struct Chat<C: CustomAgentMessages + 'static> {
    /// Page operands.
    view: Rc<DomView>,
    /// The element the caller places.
    root: Element,
    /// Shown without a session.
    empty: Element,
    /// The chat column, count control and panel placement.
    layout: Element,
    /// Holds the session view.
    main: HtmlElement,
    /// Opens the closed panel and shows the artifact count.
    pill: HtmlElement,
    /// The artifact count.
    count: Element,
    /// Places the panel beside the chat or over it.
    side: HtmlElement,
    /// The shown session.
    agent: RefCell<Option<Rc<BrowserSession<C>>>>,
    /// The session view.
    agent_interface: RefCell<Option<Rc<AgentInterface<C>>>>,
    /// The artifacts panel and its application commands.
    artifacts_panel: RefCell<Option<(Rc<ArtifactsPanel>, Rc<LocalArtifactsPanel>)>>,
    /// The panel's mounted element.
    panel_root: RefCell<MountedContent>,
    /// Whether any artifact exists.
    has_artifacts: Cell<bool>,
    /// The last seen artifact count.
    artifact_count: Cell<usize>,
    /// Whether the panel is open.
    show_artifacts_panel: Cell<bool>,
    /// The last observed window width.
    window_width: Cell<f64>,
    /// The window resize listener, while connected.
    resize: RefCell<Option<Listener>>,
    /// The count control's listener.
    open: RefCell<Option<Listener>>,
}
impl<C: CustomAgentMessages + 'static> ChatPanel<C> {
    /// Build the screen without a session.
    /// # Errors
    /// Returns a localization or native failure.
    pub fn new(view: &Rc<DomView>) -> Result<Self, ToolUiError> {
        crate::contributions::register_builtin_elements(view)
            .map_err(|error| host_error(&error))?;
        let connection: connection::Connection = Rc::default();
        let state = Rc::new(Chat::build(view, &connection)?);
        let weak = Rc::downgrade(&state);
        connection.replace(Some(Rc::new(move |connected| {
            if let Some(state) = weak.upgrade() {
                report(state.connected(connected));
            }
        })));
        state.listen()?;
        state.render()?;
        Ok(Self { state })
    }
    /// The element the caller places; it fills its parent's height.
    #[must_use]
    pub fn root(&self) -> &Element {
        &self.state.root
    }
    /// The shown session.
    #[must_use]
    pub fn agent(&self) -> Option<Rc<BrowserSession<C>>> {
        self.state.agent.borrow().clone()
    }
    /// The session view.
    #[must_use]
    pub fn agent_interface(&self) -> Option<Rc<AgentInterface<C>>> {
        self.state.agent_interface.borrow().clone()
    }
    /// The artifacts panel.
    #[must_use]
    pub fn artifacts_panel(&self) -> Option<Rc<ArtifactsPanel>> {
        let panel = self.state.artifacts_panel.borrow();
        panel.as_ref().map(|(panel, _)| Rc::clone(panel))
    }
    /// Show `agent` with a new session view and artifacts panel.
    ///
    /// The session's tools become the artifacts tool followed by the factory's tools. The
    /// artifacts its history leaves are restored without opening the panel; afterwards a new
    /// artifact opens it. Artifact rendering stays bound to this session view's panel.
    /// # Errors
    /// Returns a localization or native failure.
    pub async fn set_agent(
        &self,
        agent: Rc<BrowserSession<C>>,
        config: ChatPanelConfig<C>,
    ) -> Result<(), ToolUiError> {
        let state = &self.state;
        *state.agent.borrow_mut() = Some(Rc::clone(&agent));
        let panel = Rc::new(ArtifactsPanel::default());
        let application = panel.application(&state.view);
        let mut view = (*state.view).clone();
        Rc::make_mut(&mut view.tool_renderers).insert("artifacts".into(), renderer(&application));
        let interface = Rc::new(AgentInterface::new(
            &Rc::new(view),
            AgentInterfaceProps {
                session: Some(Rc::clone(&agent)),
                controls: EditorControls {
                    attachments: true,
                    model: true,
                    thinking: true,
                },
                show_theme_toggle: false,
                on_api_key_required: config.on_api_key_required,
                on_before_send: config.on_before_send,
                on_cost_click: config.on_cost_click,
                on_model_select: config.on_model_select,
            },
        )?);
        let session: Rc<dyn AgentLike> = Rc::clone(agent.state()) as Rc<dyn AgentLike>;
        application.set_agent(Some(&session));
        panel.set_sandbox_url_provider(config.sandbox_url_provider);
        state.observe(&panel);
        let providers = runtime_providers(Rc::downgrade(&session), Rc::clone(&application));
        let mut tools: Vec<SharedAgentTool<Local>> = vec![Rc::new(RwLock::new(application.tool()))];
        if let Some(factory) = &config.tools_factory {
            tools.extend(factory(&agent, &interface, &panel, providers));
        }
        agent
            .state()
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .set_tools(tools);
        *state.agent_interface.borrow_mut() = Some(interface);
        let previous = state
            .artifacts_panel
            .replace(Some((Rc::clone(&panel), Rc::clone(&application))));
        drop(previous);
        let messages = {
            let session = agent.state().read().unwrap_or_else(PoisonError::into_inner);
            let messages = Arc::clone(session.messages());
            drop(session);
            let messages = messages.read().unwrap_or_else(PoisonError::into_inner);
            messages.clone()
        };
        let changed = state.changed_callback();
        panel.set_on_artifacts_change(None);
        application.reconstruct_from_messages(&messages).await;
        panel.set_on_artifacts_change(Some(changed));
        let count = artifact_count(&application);
        state.has_artifacts.set(count > 0);
        state.artifact_count.set(count);
        state.render()
    }
}
/// Retain the artifacts tool renderer for one session view's panel.
fn renderer(application: &Rc<LocalArtifactsPanel>) -> ToolRendererBinding {
    let renderer = Rc::new(ArtifactsToolRenderer::new(Some(Rc::clone(application))));
    ToolRendererBinding::from_raw::<JsonObject, Value>(renderer.clone())
        .with_raw::<Value, Value>(renderer.clone())
        .with_raw::<ArtifactsParams, Value>(renderer)
}
/// The number of artifacts `application` holds.
fn artifact_count(application: &LocalArtifactsPanel) -> usize {
    application
        .artifacts()
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .len()
}
/// Runtime providers for one session and panel: its current attachments, when any, and
/// read-write artifact access recording into the session.
fn runtime_providers(
    session: Weak<dyn AgentLike>,
    application: Rc<LocalArtifactsPanel>,
) -> RuntimeProvidersFactory<BrowserSandboxValue> {
    Rc::new(move || {
        let session = session.upgrade();
        let attachments = session
            .as_ref()
            .map(|session| session.attachments())
            .unwrap_or_default();
        let mut providers: Vec<Rc<dyn SandboxRuntimeProvider<BrowserSandboxValue>>> = Vec::new();
        if !attachments.is_empty() {
            providers.push(Rc::new(AttachmentsRuntimeProvider::new(Arc::new(
                RwLock::new(attachments),
            ))));
        }
        providers.push(Rc::new(ArtifactsRuntimeProvider::new(
            &application,
            session,
            true,
        )));
        Ok(providers)
    })
}
impl<C: CustomAgentMessages + 'static> Chat<C> {
    /// Create every node once.
    fn build(view: &Rc<DomView>, connection: &connection::Connection) -> Result<Self, ToolUiError> {
        let document = &view.document;
        let root = connection::root(document, "maestro-chat-panel", "maestro-chat", connection)?;
        let empty = element(document, "div", "maestro-chat-empty")?;
        empty.set_text_content(Some("No agent set"));
        let layout = element(document, "div", "maestro-chat-layout")?;
        let main = element(document, "div", "maestro-chat-main")?.unchecked_into::<HtmlElement>();
        let side = element(document, "div", "maestro-chat-side")?.unchecked_into::<HtmlElement>();
        let (pill, count) = pill(view)?;
        append(&layout, &main)?;
        append(&layout, &pill)?;
        append(&layout, &side)?;
        Ok(Self {
            view: Rc::clone(view),
            root,
            empty,
            layout,
            main,
            pill,
            count,
            side,
            agent: RefCell::default(),
            agent_interface: RefCell::default(),
            artifacts_panel: RefCell::default(),
            panel_root: RefCell::default(),
            has_artifacts: Cell::new(false),
            artifact_count: Cell::new(0),
            show_artifacts_panel: Cell::new(false),
            window_width: Cell::new(window_width()),
            resize: RefCell::default(),
            open: RefCell::default(),
        })
    }
    /// Open the panel from the count control.
    fn listen(self: &Rc<Self>) -> Result<(), ToolUiError> {
        let weak = Rc::downgrade(self);
        let clicked = Closure::<dyn Fn(Event)>::new(move |_: Event| {
            if let Some(state) = weak.upgrade() {
                state.show_artifacts_panel.set(true);
                report(state.render());
            }
        });
        let open = EventSubscription::new(self.pill.clone().into(), "click", clicked)
            .map_err(|error| host_error(&error))?;
        self.open.replace(Some(open));
        Ok(())
    }
    /// Follow the window width only while placed in the page.
    fn connected(self: &Rc<Self>, connected: bool) -> Result<(), ToolUiError> {
        if !connected {
            let resize = self.resize.take();
            drop(resize);
            return Ok(());
        }
        let window =
            web_sys::window().ok_or_else(|| ToolUiError::Presentation("Missing window".into()))?;
        let weak = Rc::downgrade(self);
        let resized = Closure::<dyn Fn(Event)>::new(move |_: Event| {
            if let Some(state) = weak.upgrade() {
                state.window_width.set(window_width());
                report(state.render());
            }
        });
        let resize = EventSubscription::new(window.into(), "resize", resized)
            .map_err(|error| host_error(&error))?;
        self.resize.replace(Some(resize));
        self.window_width.set(window_width());
        self.render()
    }
    /// Install the panel's open, close and change notifications.
    fn observe(self: &Rc<Self>, panel: &ArtifactsPanel) {
        let weak = Rc::downgrade(self);
        panel.set_on_close(Some(Rc::new(move || {
            if let Some(state) = weak.upgrade() {
                state.show_artifacts_panel.set(false);
                report(state.render());
            }
        })));
        let weak = Rc::downgrade(self);
        panel.set_on_open(Some(Rc::new(move || {
            if let Some(state) = weak.upgrade() {
                state.show_artifacts_panel.set(true);
                report(state.render());
            }
        })));
        panel.set_on_artifacts_change(Some(self.changed_callback()));
    }
    /// Track the artifact count, opening the panel when it grows.
    fn changed_callback(self: &Rc<Self>) -> Rc<dyn Fn()> {
        let weak = Rc::downgrade(self);
        Rc::new(move || {
            let Some(state) = weak.upgrade() else {
                return;
            };
            let count = state
                .artifacts_panel
                .borrow()
                .as_ref()
                .map_or(0, |(_, application)| artifact_count(application));
            let created = count > state.artifact_count.get();
            state.has_artifacts.set(count > 0);
            state.artifact_count.set(count);
            if count > 0 && created {
                state.show_artifacts_panel.set(true);
            }
            report(state.render());
        })
    }
    /// Bring the layout in line with the session, the panel state and the window width.
    fn render(&self) -> Result<(), ToolUiError> {
        let interface = self.agent_interface.borrow().clone();
        let panel = self.artifacts_panel.borrow().clone();
        let (Some(interface), Some((panel, _))) = (interface, panel) else {
            self.layout.remove();
            return append(&self.root, &self.empty).map_err(Into::into);
        };
        self.empty.remove();
        if self.layout.parent_node().is_none() {
            append(&self.root, &self.layout)?;
        }
        if !self.main.contains(Some(interface.root())) {
            self.main.replace_children_with_node_1(interface.root());
        }
        self.panel_root
            .borrow_mut()
            .show(&*panel, self.side.as_ref(), &self.view)?;
        let narrow = self.window_width.get() < BREAKPOINT;
        let shown = self.show_artifacts_panel.get();
        let has = self.has_artifacts.get();
        panel.set_collapsed(!shown);
        panel.set_overlay(narrow);
        let split = !narrow && shown && has;
        style(&self.main, "width", if split { "50%" } else { "100%" })?;
        set_text_if_changed(&self.count, &self.artifact_count.get().to_string());
        hide(&self.pill, !has || shown)?;
        hide(&self.side, !has || !shown)?;
        for class in ["maestro-dialog-backdrop", "maestro-dialog-blur"] {
            self.side
                .class_list()
                .toggle_with_force(class, narrow)
                .map_err(|error| host_error(&error))?;
        }
        style(&self.side, "width", if narrow { "" } else { "50%" })
    }
}
/// The count control: a badge naming the artifacts and their count.
fn pill(view: &DomView) -> Result<(HtmlElement, Element), ToolUiError> {
    let document = &view.document;
    let pill = element(document, "button", "maestro-chat-pill")?.unchecked_into::<HtmlElement>();
    pill.set_attribute("type", "button")
        .map_err(|error| host_error(&error))?;
    pill.set_title(&view.i18n.text("Show artifacts")?);
    let badge = element(document, "span", "maestro-badge maestro-badge-default")?;
    let label = element(document, "span", "")?;
    label.set_text_content(Some(&view.i18n.text("Artifacts")?));
    let count = element(document, "span", "maestro-chat-pill-count")?;
    append(&badge, &label)?;
    append(&badge, &count)?;
    append(&pill, &badge)?;
    Ok((pill, count))
}
/// The window's current width in CSS pixels.
fn window_width() -> f64 {
    web_sys::window()
        .and_then(|window| window.inner_width().ok())
        .and_then(|width| width.as_f64())
        .unwrap_or_default()
}
/// Set or clear `hidden` on `element`.
fn hide(element: &Element, hidden: bool) -> Result<(), ToolUiError> {
    element
        .toggle_attribute_with_force("hidden", hidden)
        .map(drop)
        .map_err(|error| host_error(&error).into())
}
/// Set one inline style property; an empty value removes it.
fn style(element: &HtmlElement, name: &str, value: &str) -> Result<(), ToolUiError> {
    let style = element.style();
    if value.is_empty() {
        style.remove_property(name).map(drop)
    } else {
        style.set_property(name, value)
    }
    .map_err(|error| host_error(&error).into())
}
