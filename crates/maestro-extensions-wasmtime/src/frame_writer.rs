//! The frame writer of a lent terminal, kept as a container widget that also answers
//! frame-writer and terminal requests.
use crate::callback_imports::{CallbackView, optional, retain};
use crate::guest_calls::{Callback, Handle, widget_calls::Event};
use crate::import_route::Value;
use crate::local_resources::Registry;
use crate::overlay_reads::OptionsEndpoint;
use crate::widget_lists::Binding;
use crate::widget_reads::{Kind, Widget, WidgetEndpoint};
use maestro_extensions::types::UIErrorReporter;
use maestro_extensions::{Component, ComponentHandle, EventBus, InputListenerResult, Local, TUI};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
use wasmtime::component::{ComponentType, Lift, Resource};

/// What focus or an overlay goes to, as the guest sends it.
#[derive(ComponentType, Lift)]
#[component(variant)]
pub(crate) enum WireTarget {
    /// A host widget.
    #[component(name = "widget")]
    Widget(Resource<WidgetEndpoint>),
    /// An authored component lent now.
    #[component(name = "component")]
    Component(Resource<Callback>),
    /// The identity an authored component was lent under before.
    #[component(name = "lent")]
    Lent(u32),
}

/// One request on the terminal a frame writer draws on.
#[derive(ComponentType, Lift)]
#[component(variant)]
pub(crate) enum TerminalRequest {
    /// Stops the terminal.
    #[component(name = "stop")]
    Stop,
    /// Writes output.
    #[component(name = "write")]
    Write(String),
    /// Reads the width.
    #[component(name = "columns")]
    Columns,
    /// Reads the height.
    #[component(name = "rows")]
    Rows,
    /// Reads whether the Kitty keyboard protocol is active.
    #[component(name = "kitty-protocol-active")]
    KittyProtocolActive,
    /// Moves the cursor by rows.
    #[component(name = "move-by")]
    MoveBy(i64),
    /// Hides the cursor.
    #[component(name = "hide-cursor")]
    HideCursor,
    /// Shows the cursor.
    #[component(name = "show-cursor")]
    ShowCursor,
    /// Clears the current line.
    #[component(name = "clear-line")]
    ClearLine,
    /// Clears from the cursor to the end of the screen.
    #[component(name = "clear-from-cursor")]
    ClearFromCursor,
    /// Clears the screen.
    #[component(name = "clear-screen")]
    ClearScreen,
    /// Sets the window title.
    #[component(name = "set-title")]
    SetTitle(String),
    /// Turns the progress indicator on or off.
    #[component(name = "set-progress")]
    SetProgress(bool),
}

/// One frame writer request as the guest sends it.
#[derive(ComponentType, Lift)]
#[component(variant)]
pub(crate) enum WireRequest {
    /// Reads whether the hardware cursor is shown.
    #[component(name = "get-show-hardware-cursor")]
    GetShowHardwareCursor,
    /// Shows or hides the hardware cursor.
    #[component(name = "set-show-hardware-cursor")]
    SetShowHardwareCursor(bool),
    /// Reads whether shrinking content clears the screen.
    #[component(name = "get-clear-on-shrink")]
    GetClearOnShrink,
    /// Changes whether shrinking content clears the screen.
    #[component(name = "set-clear-on-shrink")]
    SetClearOnShrink(bool),
    /// Moves focus.
    #[component(name = "set-focus")]
    SetFocus(Option<WireTarget>),
    /// Shows an overlay.
    #[component(name = "show-overlay")]
    ShowOverlay((WireTarget, Option<Resource<OptionsEndpoint>>)),
    /// Removes the most recently shown overlay.
    #[component(name = "hide-overlay")]
    HideOverlay,
    /// Reads whether any overlay is visible.
    #[component(name = "has-overlay")]
    HasOverlay,
    /// Starts the terminal.
    #[component(name = "start")]
    Start,
    /// Stops the terminal.
    #[component(name = "stop")]
    Stop,
    /// Requests a frame.
    #[component(name = "request-render")]
    RequestRender(bool),
    /// Replaces or removes the debug key's callback.
    #[component(name = "set-on-debug")]
    SetOnDebug(Option<Resource<Callback>>),
    /// A request on the terminal.
    #[component(name = "terminal")]
    Terminal(TerminalRequest),
    /// Starts the terminal with authored input and resize callbacks.
    #[component(name = "terminal-start")]
    TerminalStart((Resource<Callback>, Resource<Callback>)),
    /// Reads whether the component lent under an identity is still kept.
    #[component(name = "lent-live")]
    LentLive(u32),
}

/// What focus or an overlay goes to, Store-safe.
pub(crate) enum Target {
    /// A host widget.
    Widget(WidgetEndpoint),
    /// An authored component lent now.
    Component(Handle),
    /// The identity an authored component was lent under before.
    Lent(u32),
}

/// One frame writer request, Store-safe.
pub(crate) enum Request {
    /// Reads or changes a frame writer setting, or requests a frame.
    Setting(WireRequest),
    /// Moves focus.
    SetFocus(Option<Target>),
    /// Shows an overlay.
    ShowOverlay(Target, Option<OptionsEndpoint>),
    /// Replaces or removes the debug key's callback.
    SetOnDebug(Option<Handle>),
    /// Starts the terminal with input and resize callbacks.
    TerminalStart(Handle, Handle),
}

impl WireTarget {
    /// Retains the target's resources.
    fn retained<T: CallbackView>(self, store: &mut T) -> wasmtime::Result<Target> {
        Ok(match self {
            Self::Widget(widget) => Target::Widget(store.ctx().table.get(&widget)?.clone()),
            Self::Component(component) => Target::Component(retain(store, &component)?),
            Self::Lent(id) => Target::Lent(id),
        })
    }
}

impl WireRequest {
    /// Retains the request's resources and callbacks.
    pub(crate) fn retained<T: CallbackView>(self, store: &mut T) -> wasmtime::Result<Request> {
        Ok(match self {
            Self::SetFocus(target) => {
                Request::SetFocus(target.map(|target| target.retained(store)).transpose()?)
            }
            Self::ShowOverlay((target, options)) => {
                let options = options
                    .map(|options| store.ctx().table.get(&options).cloned())
                    .transpose()?;
                Request::ShowOverlay(target.retained(store)?, options)
            }
            Self::SetOnDebug(callback) => Request::SetOnDebug(optional(store, callback.as_ref())?),
            Self::TerminalStart((input, resize)) => {
                Request::TerminalStart(retain(store, &input)?, retain(store, &resize)?)
            }
            setting => Request::Setting(setting),
        })
    }
}

/// The frame writer behind a writer widget, and the authored components it lent.
pub(crate) struct Writer {
    /// The original frame writer.
    pub(crate) tui: TUI,
    /// Authored focus and overlay targets by the identity each was lent under, held weakly so
    /// a target the frame writer no longer keeps is released.
    lent: RefCell<Vec<(u32, Weak<dyn Component>)>>,
    /// Where failed visibility checks of overlays shown here are reported.
    reporter: Option<UIErrorReporter>,
}

impl Writer {
    /// Keeps the original frame writer and the reporter of its overlays' failures.
    pub(crate) fn new(tui: TUI, reporter: Option<UIErrorReporter>) -> Self {
        Self {
            tui,
            lent: RefCell::default(),
            reporter,
        }
    }

    /// Answers one request; `children` resolves identities of authored children.
    pub(crate) fn answer(
        &self,
        request: Request,
        binding: &Binding<'_>,
        children: &dyn Fn(u32) -> Option<ComponentHandle>,
    ) -> Result<Value, String> {
        let tui = &self.tui;
        match request {
            Request::Setting(setting) => self.setting(setting),
            Request::SetFocus(target) => {
                let target = target
                    .map(|target| self.component(target, binding, children))
                    .transpose()?;
                tui.set_focus(target);
                Ok(Value::Unit)
            }
            Request::ShowOverlay(target, options) => {
                let component = self.component(target, binding, children)?;
                let options = options
                    .map(|options| {
                        binding
                            .registry
                            .overlay_options(&options, self.reporter.as_ref())
                    })
                    .transpose()?;
                let shown = tui
                    .show_overlay(component, options)
                    .map_err(|error| error.to_string())?;
                let handle = binding
                    .registry
                    .overlay_handle(Rc::new(RefCell::new(shown)))?;
                Ok(Value::Overlay(handle))
            }
            Request::SetOnDebug(callback) => {
                tui.set_on_debug(callback.map(|callback| notice(binding, callback)));
                Ok(Value::Unit)
            }
            Request::TerminalStart(input, resize) => {
                let on_input = text(binding, input);
                let on_resize = notice(binding, resize);
                let started = tui.terminal().borrow_mut().start(
                    Box::new(move |data: &str| on_input(data)),
                    Box::new(move || on_resize()),
                );
                started
                    .map(|()| Value::Unit)
                    .map_err(|error| error.to_string())
            }
        }
    }

    /// Reads or changes a setting, starts or stops the frame writer, or runs a terminal
    /// request.
    fn setting(&self, request: WireRequest) -> Result<Value, String> {
        let tui = &self.tui;
        let done = |result: std::io::Result<()>| {
            result
                .map(|()| Value::Unit)
                .map_err(|error| error.to_string())
        };
        match request {
            WireRequest::GetShowHardwareCursor => {
                Ok(Value::Boolean(tui.get_show_hardware_cursor()))
            }
            WireRequest::SetShowHardwareCursor(enabled) => {
                done(tui.set_show_hardware_cursor(enabled))
            }
            WireRequest::GetClearOnShrink => Ok(Value::Boolean(tui.get_clear_on_shrink())),
            WireRequest::SetClearOnShrink(enabled) => {
                tui.set_clear_on_shrink(enabled);
                Ok(Value::Unit)
            }
            WireRequest::HideOverlay => done(tui.hide_overlay()),
            WireRequest::HasOverlay => Ok(Value::Boolean(tui.has_overlay())),
            WireRequest::Start => done(tui.start()),
            WireRequest::Stop => done(tui.stop()),
            WireRequest::RequestRender(force) => {
                tui.request_render(force);
                Ok(Value::Unit)
            }
            WireRequest::Terminal(request) => terminal(tui, request),
            WireRequest::LentLive(id) => Ok(Value::Boolean(self.lent(id).is_some())),
            _ => Err("expected a frame writer setting".to_owned()),
        }
    }

    /// The component a target names; an authored component lent now is kept under its
    /// identity.
    fn component(
        &self,
        target: Target,
        binding: &Binding<'_>,
        children: &dyn Fn(u32) -> Option<ComponentHandle>,
    ) -> Result<ComponentHandle, String> {
        match target {
            Target::Widget(widget) => binding
                .registry
                .widget(&widget)
                .map(|widget| widget.component()),
            Target::Component(handle) => {
                let id = handle.id();
                let view = crate::guest_component::GuestComponent::new(
                    handle,
                    binding.registry.clone(),
                    binding.events.clone(),
                );
                let view: ComponentHandle = Rc::new(view);
                let mut lent = self.lent.borrow_mut();
                lent.retain(|(_, view)| view.strong_count() > 0);
                lent.push((id, Rc::downgrade(&view)));
                Ok(view)
            }
            Target::Lent(id) => self
                .lent(id)
                .or_else(|| children(id))
                .ok_or_else(|| "extension component was not lent to this terminal".to_owned()),
        }
    }

    /// The target lent under `id`, while something else still keeps it.
    fn lent(&self, id: u32) -> Option<ComponentHandle> {
        self.lent
            .borrow()
            .iter()
            .find(|(lent, _)| *lent == id)
            .and_then(|(_, view)| view.upgrade())
    }
}

/// Runs one request on the terminal.
fn terminal(tui: &TUI, request: TerminalRequest) -> Result<Value, String> {
    let mut terminal = tui
        .terminal()
        .try_borrow_mut()
        .map_err(|_| "the terminal is already in use".to_owned())?;
    let count = |count: usize| {
        u32::try_from(count)
            .map(Value::Count)
            .map_err(|error| error.to_string())
    };
    let result = match request {
        TerminalRequest::Columns => return count(terminal.columns()),
        TerminalRequest::Rows => return count(terminal.rows()),
        TerminalRequest::KittyProtocolActive => {
            return Ok(Value::Boolean(terminal.kitty_protocol_active()));
        }
        TerminalRequest::Stop => terminal.stop(),
        TerminalRequest::Write(data) => terminal.write(&data),
        TerminalRequest::MoveBy(lines) => isize::try_from(lines)
            .map_err(std::io::Error::other)
            .and_then(|lines| terminal.move_by(lines)),
        TerminalRequest::HideCursor => terminal.hide_cursor(),
        TerminalRequest::ShowCursor => terminal.show_cursor(),
        TerminalRequest::ClearLine => terminal.clear_line(),
        TerminalRequest::ClearFromCursor => terminal.clear_from_cursor(),
        TerminalRequest::ClearScreen => terminal.clear_screen(),
        TerminalRequest::SetTitle(title) => terminal.set_title(&title),
        TerminalRequest::SetProgress(active) => terminal.set_progress(active),
    };
    result
        .map(|()| Value::Unit)
        .map_err(|error| error.to_string())
}

/// An authored callback without operands; its failure is kept for the writer's next request.
fn notice(binding: &Binding<'_>, handle: Handle) -> Rc<dyn Fn()> {
    let (registry, events, failure) = binding.owned();
    Rc::new(move || {
        let binding = Binding {
            registry: &registry,
            events: &events,
            failure: &failure,
        };
        binding.call(&handle, Event::Notice);
    })
}

/// An authored text callback; its failure is kept for the writer's next request.
fn text(binding: &Binding<'_>, handle: Handle) -> Rc<dyn Fn(&str)> {
    let (registry, events, failure) = binding.owned();
    Rc::new(move |data: &str| {
        let binding = Binding {
            registry: &registry,
            events: &events,
            failure: &failure,
        };
        binding.call(&handle, Event::Text(data.to_owned()));
    })
}

/// An authored input listener; a failed chunk is consumed and its failure kept for the
/// writer's next request.
pub(crate) fn listener(
    binding: &Binding<'_>,
    handle: Handle,
) -> Rc<dyn Fn(&str) -> InputListenerResult> {
    let (registry, events, failure) = binding.owned();
    Rc::new(move |data: &str| {
        let decided = events.invoke_callback(|emitter| {
            let emitter = registry.emitter(emitter.frame())?;
            handle
                .decide(&registry, emitter, data.to_owned())
                .map_err(maestro_extensions::EventError::from)
        });
        decided.unwrap_or_else(|error| {
            failure.borrow_mut().get_or_insert(error.to_string());
            InputListenerResult::Consume
        })
    })
}

impl Widget {
    /// The writer widget of a lent frame writer whose authored callbacks enter `events` and
    /// whose overlays' failures go to the reporter it was lent with.
    pub(crate) fn writer(
        (tui, reporter): (TUI, Option<UIErrorReporter>),
        events: EventBus<Local>,
    ) -> Self {
        let component: ComponentHandle = Rc::new(tui.clone());
        Self {
            kind: Kind::Writer(Rc::new(Writer::new(tui, reporter)), component),
            failure: crate::guest_calls::paints::Failure::default(),
            events,
            guests: RefCell::default(),
            nested: std::cell::OnceCell::new(),
        }
    }

    /// The original frame writer, when the widget is a writer widget.
    pub(crate) fn tui(&self) -> Option<&maestro_extensions::TUI> {
        match &self.kind {
            Kind::Writer(writer, _) => Some(&writer.tui),
            _ => None,
        }
    }

    /// Answers a frame writer request, then reports an authored callback's failure.
    pub(crate) fn writer_request(
        &self,
        request: crate::frame_writer::Request,
        registry: &Registry,
    ) -> Result<crate::import_route::Value, String> {
        let Kind::Writer(writer, _) = &self.kind else {
            return Err("the widget is no frame writer".to_owned());
        };
        let children = |id: u32| {
            let guests = self.guests.borrow();
            guests
                .iter()
                .find_map(|(added, view)| (*added == id).then(|| view.clone()))
        };
        let answer = writer.answer(request, &self.binding(registry), &children);
        let failed = self.failure.borrow_mut().take();
        failed.map_or(answer, Err)
    }

    /// Adds an authored input listener to the frame writer and retains its remover.
    pub(crate) fn add_input_listener(
        &self,
        handle: Handle,
        registry: &Registry,
    ) -> Result<crate::local_resources::effects::SubscriptionEndpoint, String> {
        let Kind::Writer(writer, _) = &self.kind else {
            return Err("the widget is no frame writer".to_owned());
        };
        let listener = crate::frame_writer::listener(&self.binding(registry), handle);
        let remove = writer.tui.add_input_listener(listener);
        registry.ui_subscription(Rc::new(move || {
            remove();
            Ok(())
        }))
    }
}
