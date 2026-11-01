//! Native session view nodes, rendering and reader-follow.
use super::{AgentInterfaceProps, BrowserSession, Follow, usage_totals};
use crate::{
    MessageList, SharedAttachment, StreamingMessageContainer,
    browser::{
        attachment_content::browser::{append, element, host_error, set_text_if_changed},
        event_subscription::EventSubscription,
    },
    components::{
        connection,
        message_editor::{MessageEditor, MessageEditorProps},
    },
    dom::{DomContent, DomRoot, DomView, MountedContent, report},
    tools::types::ToolUiError,
    utils::format::format_usage,
};
use maestro_app::browser::components::messages::CustomAgentMessages as BrowserMessages;
use maestro_app::{AgentMessage, CustomAgentMessages, Local, NoCustomMessages, Subscription};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{Arc, PoisonError, RwLock},
};
use wasm_bindgen::{JsCast as _, closure::Closure};
use web_sys::{Element, Event, ResizeObserver};

/// One retained native listener.
type Listener = EventSubscription<Closure<dyn Fn(Event)>>;
/// The content resize observation and its callback.
type Observation = (ResizeObserver, Closure<dyn Fn()>);

/// The retained session view; dropping it ends its subscription and observations.
pub struct AgentInterface<C: CustomAgentMessages + 'static = NoCustomMessages> {
    /// Shared state the listeners reach weakly.
    pub(super) state: Rc<Session<C>>,
}
/// Nodes and view state retained across updates.
pub(super) struct Session<C: CustomAgentMessages + 'static> {
    /// Page operands.
    pub(super) view: Rc<DomView>,
    /// Last supplied operands.
    pub(super) props: RefCell<AgentInterfaceProps<C>>,
    /// The element the caller places.
    pub(super) root: Element,
    /// Shown without a session.
    empty: Element,
    /// Transcript and input column, shown with a session.
    layout: Element,
    /// Scrolling transcript area.
    scroll: Element,
    /// Centered transcript content.
    content: Element,
    /// Stable history placement.
    list: RefCell<MountedContent>,
    /// Parent of the stable history.
    history: Element,
    /// The streaming response.
    pub(super) stream: RefCell<StreamingMessageContainer<C>>,
    /// The streaming response's mounted root.
    stream_root: RefCell<Option<DomRoot>>,
    /// Hidden while nothing streams.
    stream_parent: Element,
    /// The message editor.
    pub(super) editor: MessageEditor,
    /// The editor operands carrying this view's callbacks.
    pub(super) editor_props: RefCell<MessageEditorProps>,
    /// The totals row.
    stats: Element,
    /// The appearance button, while shown.
    theme: RefCell<Option<crate::ThemeToggle>>,
    /// Usage totals as text.
    usage: Element,
    /// Usage totals as an action.
    usage_action: Element,
    /// Reader-follow state.
    pub(super) follow: Cell<Follow>,
    /// Set from the run's end event until the next run starts: the session reports streaming
    /// until its listeners return.
    pub(super) ended: Cell<bool>,
    /// The live session subscription.
    pub(super) subscription: RefCell<Option<Subscription<BrowserMessages<C>, Local>>>,
    /// Scroll and usage listeners.
    listeners: RefCell<Vec<Listener>>,
    /// Content size observation.
    observation: RefCell<Option<Observation>>,
}
impl<C: CustomAgentMessages + 'static> AgentInterface<C> {
    /// Build the view and bind it to the supplied session.
    /// # Errors
    /// Returns a localization, icon or native failure.
    pub fn new(view: &Rc<DomView>, props: AgentInterfaceProps<C>) -> Result<Self, ToolUiError> {
        crate::dom::install_styles(view).map_err(|error| host_error(&error))?;
        let connection = Rc::default();
        let state = Rc::new(Session::build(view, props, &connection)?);
        let weak = Rc::downgrade(&state);
        connection.replace(Some(Rc::new(move |connected| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            if connected {
                report(state.bind());
                state.subscribe();
                report(state.render());
            } else {
                state.disconnect();
            }
        })));
        state.bind_editor();
        state.subscribe();
        state.render()?;
        Ok(Self { state })
    }
    /// The element the caller places; it fills its parent's height.
    #[must_use]
    pub fn root(&self) -> &Element {
        &self.state.root
    }
    /// Apply new operands; a different session moves the subscription to it.
    /// # Errors
    /// Returns a localization or native failure.
    pub fn update(&self, props: AgentInterfaceProps<C>) -> Result<(), ToolUiError> {
        let changed = {
            let current = self.state.props.borrow();
            match (&current.session, &props.session) {
                (Some(old), Some(new)) => !Rc::ptr_eq(old, new),
                (None, None) => false,
                _ => true,
            }
        };
        let previous = self.state.props.replace(props);
        drop(previous);
        if changed {
            self.state.subscribe();
        }
        self.state.bind_editor();
        self.state.render()
    }
    /// Replace the editor's draft text and attachments.
    /// # Errors
    /// Returns a localization, attachment or native failure.
    pub fn set_input(
        &self,
        text: &str,
        attachments: Vec<SharedAttachment>,
    ) -> Result<(), ToolUiError> {
        self.state.editor.set_value(text)?;
        self.state.editor.set_attachments(attachments)
    }
    /// Turn following new content on or off.
    pub fn set_auto_scroll(&self, enabled: bool) {
        let mut follow = self.state.follow.get();
        follow.enabled = enabled;
        self.state.follow.set(follow);
    }
    /// The message editor.
    #[must_use]
    pub fn editor(&self) -> &MessageEditor {
        &self.state.editor
    }
}
impl<C: CustomAgentMessages + 'static> Drop for AgentInterface<C> {
    fn drop(&mut self) {
        self.state.disconnect();
    }
}
impl<C: CustomAgentMessages + 'static> Session<C> {
    /// Create every node once.
    fn build(
        view: &Rc<DomView>,
        props: AgentInterfaceProps<C>,
        connection: &connection::Connection,
    ) -> Result<Self, ToolUiError> {
        let document = &view.document;
        let root = connection::root(
            document,
            "maestro-agent-interface",
            "maestro-agent",
            connection,
        )?;
        let empty = element(document, "div", "maestro-agent-empty")?;
        let layout = element(document, "div", "maestro-agent-layout")?;
        let scroll = element(document, "div", "maestro-agent-scroll")?;
        let content = element(document, "div", "maestro-agent-content")?;
        let column = element(document, "div", "maestro-agent-column")?;
        let history = element(document, "div", "")?;
        let stream_parent = element(document, "div", "")?;
        let input = element(document, "div", "maestro-agent-input")?;
        let inner = element(document, "div", "maestro-agent-inner")?;
        let stats = element(document, "div", "maestro-agent-stats")?;
        let (usage, usage_action) = usage_controls(document)?;
        let editor = MessageEditor::new(view, MessageEditorProps::default())?;
        append(&column, &history)?;
        append(&column, &stream_parent)?;
        append(&content, &column)?;
        append(&scroll, &content)?;
        append(&stats, &usage)?;
        append(&inner, editor.root())?;
        append(&inner, &stats)?;
        append(&input, &inner)?;
        append(&layout, &scroll)?;
        append(&layout, &input)?;
        let stream = StreamingMessageContainer::default();
        let stream_root = stream.mount(view)?;
        stream_parent
            .append_child(&stream_root.node)
            .map_err(|error| host_error(&error))?;
        Ok(Self {
            view: Rc::clone(view),
            props: RefCell::new(props),
            root,
            empty,
            layout,
            scroll,
            content,
            list: RefCell::default(),
            history,
            stream: RefCell::new(stream),
            stream_root: RefCell::new(Some(stream_root)),
            stream_parent,
            editor,
            editor_props: RefCell::default(),
            stats,
            theme: RefCell::default(),
            usage,
            usage_action,
            follow: Cell::default(),
            ended: Cell::new(false),
            subscription: RefCell::default(),
            listeners: RefCell::default(),
            observation: RefCell::default(),
        })
    }
    /// Follow the reader's scrolling and keep the bottom in view while following.
    fn bind(self: &Rc<Self>) -> Result<(), ToolUiError> {
        let weak = Rc::downgrade(self);
        let scrolled = Closure::<dyn Fn(Event)>::new(move |_: Event| {
            if let Some(state) = weak.upgrade() {
                state.scrolled();
            }
        });
        let scroll = EventSubscription::new(self.scroll.clone().into(), "scroll", scrolled)
            .map_err(|error| host_error(&error))?;
        let weak = Rc::downgrade(self);
        let clicked = Closure::<dyn Fn(Event)>::new(move |_: Event| {
            let action = weak
                .upgrade()
                .and_then(|state| state.props.borrow().on_cost_click.clone());
            if let Some(action) = action {
                action();
            }
        });
        let usage = EventSubscription::new(self.usage_action.clone().into(), "click", clicked)
            .map_err(|error| host_error(&error))?;
        self.listeners.replace(vec![scroll, usage]);
        let weak = Rc::downgrade(self);
        let resized = Closure::<dyn Fn()>::new(move || {
            if let Some(state) = weak.upgrade() {
                state.keep_bottom();
            }
        });
        let observer =
            ResizeObserver::new(resized.as_ref().unchecked_ref()).map_err(|e| host_error(&e))?;
        observer.observe(&self.content);
        self.observation.replace(Some((observer, resized)));
        Ok(())
    }
    /// Release native observations and subscriptions while retaining the draft and nodes.
    fn disconnect(&self) {
        self.unsubscribe();
        let listeners = self.listeners.take();
        let observation = self.observation.take();
        if let Some((observer, _)) = &observation {
            observer.disconnect();
        }
        drop((listeners, observation));
    }
    /// Record one scroll observation, with fractional offsets.
    fn scrolled(&self) {
        let top = js_sys::Reflect::get(&self.scroll, &"scrollTop".into())
            .ok()
            .and_then(|value| value.as_f64())
            .unwrap_or_default();
        let mut follow = self.follow.get();
        follow.scrolled(
            top,
            f64::from(self.scroll.scroll_height()),
            f64::from(self.scroll.client_height()),
        );
        self.follow.set(follow);
    }
    /// Scroll to the bottom while following.
    pub(super) fn keep_bottom(&self) {
        if self.follow.get().enabled {
            self.scroll.set_scroll_top(self.scroll.scroll_height());
        }
    }
    /// The supplied session.
    pub(super) fn session(&self) -> Option<Rc<BrowserSession<C>>> {
        self.props.borrow().session.clone()
    }
    /// Bring the transcript, the stream, the editor and the totals in line with the session.
    pub(super) fn render(&self) -> Result<(), ToolUiError> {
        let Some(session) = self.session() else {
            set_text_if_changed(&self.empty, &self.view.i18n.text("No session set")?);
            self.layout.remove();
            return append(&self.root, &self.empty).map_err(Into::into);
        };
        self.empty.remove();
        if self.layout.parent_node().is_none() {
            append(&self.root, &self.layout)?;
        }
        let snapshot = Snapshot::read(&session, self.ended.get());
        self.render_history(&session, &snapshot)?;
        self.render_stream(&session, &snapshot)?;
        self.render_editor(&session, snapshot.is_streaming)?;
        self.render_theme()?;
        self.render_usage(&snapshot)
    }
    /// Show the appearance button at the start of the totals row while it is requested.
    fn render_theme(&self) -> Result<(), ToolUiError> {
        let shown = self.props.borrow().show_theme_toggle;
        let mut theme = self.theme.borrow_mut();
        match (shown, theme.as_ref()) {
            (true, None) => {
                let toggle = crate::ThemeToggle::new(&self.view, false)?;
                self.stats
                    .prepend_with_node_1(toggle.root())
                    .map_err(|error| host_error(&error))?;
                *theme = Some(toggle);
            }
            (false, Some(toggle)) => {
                toggle.root().remove();
                *theme = None;
            }
            _ => {}
        }
        Ok(())
    }
    /// Show the completed history.
    fn render_history(
        &self,
        session: &BrowserSession<C>,
        snapshot: &Snapshot,
    ) -> Result<(), ToolUiError> {
        let state = session
            .state()
            .read()
            .unwrap_or_else(PoisonError::into_inner);
        let list = MessageList::<C> {
            messages: Arc::clone(state.messages()),
            tools: Rc::clone(state.tools()),
            pending_tool_calls: Some(Rc::clone(&snapshot.pending)),
            is_streaming: snapshot.is_streaming,
            on_cost_click: self.props.borrow().on_cost_click.clone(),
        };
        drop(state);
        self.list
            .borrow_mut()
            .show(&list, self.history.as_ref(), &self.view)
    }
    /// Apply the session's live operands to the streaming container.
    fn render_stream(
        &self,
        session: &BrowserSession<C>,
        snapshot: &Snapshot,
    ) -> Result<(), ToolUiError> {
        self.stream_parent
            .toggle_attribute_with_force("hidden", !snapshot.is_streaming)
            .map_err(|error| host_error(&error))?;
        let tools = Rc::clone(
            session
                .state()
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .tools(),
        );
        let results = Rc::new(results(session));
        {
            let mut stream = self.stream.borrow_mut();
            stream.tools = tools;
            stream.is_streaming = snapshot.is_streaming;
            stream.pending_tool_calls = Some(Rc::clone(&snapshot.pending));
            stream.tool_results_by_id = Some(results);
            stream
                .on_cost_click
                .clone_from(&self.props.borrow().on_cost_click);
        }
        let root = self.stream_root.borrow();
        match root.as_ref() {
            Some(root) => self.stream.borrow().update(root, &self.view),
            None => Ok(()),
        }
    }
    /// Pass the session's model, level and streaming state to the editor.
    fn render_editor(
        &self,
        session: &BrowserSession<C>,
        is_streaming: bool,
    ) -> Result<(), ToolUiError> {
        let (model, level) = {
            let state = session
                .state()
                .read()
                .unwrap_or_else(PoisonError::into_inner);
            let model = state
                .model
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .clone();
            (model, state.thinking_level.clone())
        };
        let mut props = self.editor_props.borrow().clone();
        props.controls = self.props.borrow().controls;
        props.is_streaming = is_streaming;
        props.current_model = Some(model);
        props.thinking_level = level;
        self.editor.update(props)
    }
    /// Show the summed usage, as an action when the caller supplied one.
    fn render_usage(&self, snapshot: &Snapshot) -> Result<(), ToolUiError> {
        let text = format_usage(snapshot.totals.as_ref());
        let action = self.props.borrow().on_cost_click.is_some();
        let (shown, hidden) = if action {
            (&self.usage_action, &self.usage)
        } else {
            (&self.usage, &self.usage_action)
        };
        hidden.remove();
        set_text_if_changed(shown, &text);
        if text.is_empty() {
            shown.remove();
        } else if shown.parent_node().is_none() {
            append(&self.stats, shown)?;
        }
        Ok(())
    }
}
/// The usage totals as text and as an action.
fn usage_controls(document: &web_sys::Document) -> Result<(Element, Element), ToolUiError> {
    let usage = element(document, "span", "maestro-agent-usage")?;
    let action = element(
        document,
        "button",
        "maestro-agent-usage maestro-agent-usage-action",
    )?;
    action
        .set_attribute("type", "button")
        .map_err(|error| host_error(&error))?;
    Ok((usage, action))
}
/// Session values read once per render.
struct Snapshot {
    /// Whether a response streams.
    is_streaming: bool,
    /// Calls waiting for results.
    pending: Rc<HashSet<String>>,
    /// Summed assistant usage.
    totals: Option<maestro_app::Usage>,
}
impl Snapshot {
    /// Read the session's live values without holding its state; an ended run is not streaming.
    fn read<C: CustomAgentMessages>(session: &BrowserSession<C>, ended: bool) -> Self {
        let state = session
            .state()
            .read()
            .unwrap_or_else(PoisonError::into_inner);
        let messages = Arc::clone(state.messages());
        let snapshot = Self {
            is_streaming: state.is_streaming() && !ended,
            pending: Rc::new(state.pending_tool_calls().iter().cloned().collect()),
            totals: None,
        };
        drop(state);
        let totals = usage_totals(&messages.read().unwrap_or_else(PoisonError::into_inner));
        Self { totals, ..snapshot }
    }
}
/// The session's tool results by call identifier.
fn results<C: CustomAgentMessages>(
    session: &BrowserSession<C>,
) -> HashMap<String, Arc<RwLock<maestro_app::ToolResultMessage>>> {
    let messages = Arc::clone(
        session
            .state()
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .messages(),
    );
    let messages = messages.read().unwrap_or_else(PoisonError::into_inner);
    messages
        .iter()
        .filter_map(|message| match message {
            AgentMessage::ToolResult(result) => {
                let id = result
                    .read()
                    .unwrap_or_else(PoisonError::into_inner)
                    .tool_call_id
                    .clone();
                Some((id, Arc::clone(result)))
            }
            _ => None,
        })
        .collect()
}
