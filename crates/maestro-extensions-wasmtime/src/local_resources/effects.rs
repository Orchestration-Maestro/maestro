//! Native callback operands retained by the same capability registry.
use super::{Capability, Lease, Message, Registry, pump::State};
use maestro_extensions::callbacks::CallbackFrame;
use maestro_extensions::{AgentToolUpdateCallback, LiveTheme, Local, Subscription};
use std::rc::Rc;
use std::sync::{Arc, mpsc};

pub(crate) mod emission;

/// Catchable results returned to the waiting Store thread.
type Reply = mpsc::Sender<Result<(), String>>;

/// Operations on independently retained native callback operands.
pub(crate) enum Request {
    /// Traverses a retained listener snapshot using the same receiver.
    Emission(emission::Request),
    /// Redraws a tool row.
    Invalidate(Reply),
    /// Delivers a decoded partial result to the original observer.
    Update(String, Reply),
    /// Removes exactly the native registration, without cancelling its tails.
    Unsubscribe(Reply),
    /// Supplies the original listener's independently owned guest continuation.
    Tail(crate::guest_calls::Handle, Reply),
}

/// Independently retained progress observer, not the context that supplied it.
#[derive(Clone)]
pub(crate) struct ToolUpdate(Arc<Lease>);

impl ToolUpdate {
    /// Validates a native result before invoking its observer on the local owner.
    pub(crate) fn update(&self, partial: String) -> crate::import_route::Route {
        self.0
            .route(|id, reply| Message::Effect(id, Request::Update(partial, reply)))
    }
}

/// Independently retained explicit remover; dropping it does not unsubscribe.
#[derive(Clone)]
pub(crate) struct SubscriptionEndpoint(pub(super) Arc<Lease>);

impl SubscriptionEndpoint {
    /// Applies the native owner's idempotent removal policy.
    pub(crate) fn unsubscribe(&self) -> Result<(), String> {
        self.0
            .request(|id, reply| Message::Effect(id, Request::Unsubscribe(reply)))
    }
}

/// Live theme endpoint retained independently of its renderer.
pub(crate) use crate::theme_reads::Endpoint as ThemeEndpoint;

/// Redraw capability of one tool row.
#[derive(Clone)]
pub(crate) struct InvalidatorEndpoint(pub(super) Arc<Lease>);

impl InvalidatorEndpoint {
    /// Redraws the row on the local owner.
    pub(crate) fn invalidate(&self) -> Result<(), String> {
        self.0
            .request(|id, reply| Message::Effect(id, Request::Invalidate(reply)))
    }
}

/// Terminal lent to one component factory; it keeps the native terminal while the guest holds it.
#[derive(Clone)]
pub(crate) struct TuiEndpoint {
    /// Releases the retained terminal with the last holder.
    pub(super) lease: Arc<Lease>,
}

impl TuiEndpoint {
    /// Reads the original frame writer, without keeping a second counter.
    pub(crate) fn full_redraws(&self) -> Result<u64, String> {
        self.lease.request(|id, reply| {
            Message::Widgets(crate::widget_reads::WidgetMessage::FullRedraws(id, reply))
        })
    }
}

/// Store-safe endpoint of the original native callback frame, not a second event bus.
#[derive(Clone)]
pub(crate) struct EmitterEndpoint(Arc<Lease>);

impl EmitterEndpoint {
    /// Announces a tail while this listener prefix still owns its frame.
    pub(crate) fn start_tail(&self, tail: crate::guest_calls::Handle) -> Result<(), String> {
        self.0
            .request(|id, reply| Message::Effect(id, Request::Tail(tail, reply)))
    }
}

impl Registry {
    /// Retains the original invocation frame without extending its validity.
    pub(crate) fn emitter(&self, frame: CallbackFrame<Local>) -> Result<EmitterEndpoint, String> {
        self.insert(Capability::CallbackFrame(Frame {
            native: frame,
            snapshots: std::cell::RefCell::default(),
            local: std::cell::RefCell::default(),
            tail: std::cell::RefCell::new(Tail::Open(None)),
        }))
        .map(EmitterEndpoint)
    }

    /// Transfers the announced continuation once, leaving the prefix closed to later tails.
    pub(crate) fn take_tail(
        &self,
        endpoint: &EmitterEndpoint,
    ) -> Result<Option<maestro_extensions::EventTail<Local>>, String> {
        let capability = self.capability(endpoint.0.id);
        match capability.as_deref() {
            Some(Capability::CallbackFrame(frame)) => frame.take_tail(),
            _ => Err("extension callback-emitter resource ended".to_owned()),
        }
    }

    /// Retains the original progress observer rather than a new tool runner.
    pub(crate) fn tool_update(
        &self,
        update: AgentToolUpdateCallback<serde_json::Value, Local>,
    ) -> Result<ToolUpdate, String> {
        self.insert(Capability::ToolUpdate(update)).map(ToolUpdate)
    }

    /// Retains the actual remover returned by the existing event bus.
    pub(crate) fn subscription(
        &self,
        subscription: Subscription<Local>,
    ) -> Result<SubscriptionEndpoint, String> {
        self.insert(Capability::Subscription(subscription))
            .map(SubscriptionEndpoint)
    }

    /// The live theme behind a retained theme resource.
    pub(crate) fn live_theme(&self, theme: &ThemeEndpoint) -> Result<LiveTheme, String> {
        match self.capability(theme.0.id).as_deref() {
            Some(Capability::Theme(theme)) => Ok(theme.clone()),
            _ => Err("extension theme resource ended".to_owned()),
        }
    }

    /// Adds an authored terminal input listener to the original UI and retains its remover.
    pub(crate) fn terminal_input(
        &self,
        ui: &maestro_extensions::ExtensionUIContext,
        handler: crate::guest_calls::Handle,
        api: &crate::api_reads::Endpoint,
    ) -> Result<SubscriptionEndpoint, String> {
        let listener = handler.terminal_input(self.clone(), &self.facade(api)?);
        let remove = ui
            .on_terminal_input(listener)
            .map_err(|error| error.to_string())?;
        self.ui_subscription(remove)
    }

    /// Retains the explicit remover of a terminal input listener.
    pub(crate) fn ui_subscription(
        &self,
        remove: Rc<dyn Fn() -> Result<(), maestro_extensions::EventError>>,
    ) -> Result<SubscriptionEndpoint, String> {
        self.insert(Capability::UiSubscription(remove))
            .map(SubscriptionEndpoint)
    }

    /// Retains the redraw capability of one tool row.
    pub(crate) fn invalidator(
        &self,
        invalidate: Rc<dyn Fn()>,
    ) -> Result<InvalidatorEndpoint, String> {
        self.insert(Capability::Invalidate(invalidate))
            .map(InvalidatorEndpoint)
    }

    /// Retains the native footer metadata lent to one footer factory.
    pub(crate) fn footer(
        &self,
        footer: Rc<dyn maestro_extensions::footer_data_provider::ReadonlyFooterDataProvider>,
    ) -> Result<crate::footer_data::Endpoint, String> {
        self.insert(Capability::Footer(footer))
            .map(crate::footer_data::Endpoint)
    }

    /// Retains a removal operation that leaves its observer when only dropped.
    pub(crate) fn subscription_remover(
        &self,
        remove: Rc<dyn Fn()>,
    ) -> Result<SubscriptionEndpoint, String> {
        self.insert(Capability::SignalSubscription(remove))
            .map(SubscriptionEndpoint)
    }

    /// Retains the native terminal lent to one component factory, with the reporter of
    /// failures of the callbacks it runs.
    pub(crate) fn tui(
        &self,
        tui: maestro_extensions::TUI,
        reporter: Option<maestro_extensions::types::UIErrorReporter>,
    ) -> Result<TuiEndpoint, String> {
        self.insert(Capability::Tui(tui, reporter))
            .map(|lease| TuiEndpoint { lease })
    }

    /// Retains the native theme supplied to one renderer invocation.
    pub(crate) fn theme(&self, theme: LiveTheme) -> Result<ThemeEndpoint, String> {
        self.insert(Capability::Theme(theme)).map(ThemeEndpoint)
    }
}

impl State {
    /// Calls native code without holding the capability-table borrow.
    pub(super) fn effect(&self, id: u64, request: Request) {
        let capability = self.registry.capability(id);
        match request {
            Request::Emission(request) => self.emission(capability.as_deref(), request),
            Request::Tail(tail, reply) => {
                let result = match capability.as_deref() {
                    Some(Capability::CallbackFrame(frame)) => {
                        frame.start_tail(tail, self.registry.clone())
                    }
                    _ => Err("extension callback-emitter resource ended".to_owned()),
                };
                let _ = reply.send(result);
            }
            Request::Invalidate(reply) => {
                let result = match capability.as_deref() {
                    Some(Capability::Invalidate(invalidate)) => {
                        invalidate();
                        Ok(())
                    }
                    _ => Err("extension tool-invalidator resource ended".to_owned()),
                };
                let _ = reply.send(result);
            }
            Request::Update(partial, reply) => {
                let result = match capability.as_deref() {
                    Some(Capability::ToolUpdate(update)) => {
                        crate::writer_reads::decode(&partial).map(|partial| update(partial))
                    }
                    _ => Err("extension tool-update resource ended".to_owned()),
                };
                let _ = reply.send(result);
            }
            Request::Unsubscribe(reply) => {
                let result = match capability.as_deref() {
                    Some(Capability::Subscription(subscription)) => {
                        subscription.unsubscribe();
                        Ok(())
                    }
                    Some(Capability::SignalSubscription(remove)) => {
                        remove();
                        Ok(())
                    }
                    Some(Capability::UiSubscription(remove)) => {
                        remove().map_err(|error| error.to_string())
                    }
                    _ => Err("extension subscription resource ended".to_owned()),
                };
                let _ = reply.send(result);
            }
        }
    }
}

/// One original native emission frame with a single transferable listener continuation.
pub(super) struct Frame {
    /// Native owner alone controls callback validity and foreign delivery timing.
    native: CallbackFrame<Local>,
    /// Only guest-local prefixes need an explicit split invocation owner.
    local: std::cell::RefCell<Option<maestro_extensions::callbacks::ListenerInvocation<Local>>>,
    /// Nested emissions retain independent immutable snapshots.
    snapshots: std::cell::RefCell<emission::Snapshots>,
    /// Only a live listener prefix can announce its independently owned tail.
    tail: std::cell::RefCell<Tail>,
}

/// Listener prefix ownership, distinct from whether it announced a continuation.
enum Tail {
    /// A prefix may announce one owned continuation before returning.
    Open(Option<maestro_extensions::EventTail<Local>>),
    /// The native listener has already taken its continuation.
    Closed,
}

impl Frame {
    /// Retains the guest continuation for the native bus, without polling it early.
    fn start_tail(
        &self,
        tail: crate::guest_calls::Handle,
        registry: Registry,
    ) -> Result<(), String> {
        let mut slot = self.tail.borrow_mut();
        match &mut *slot {
            Tail::Open(slot) if slot.is_none() => {
                *slot = Some(Box::pin(async move {
                    tail.tail(&registry).await.map_err(Into::into)
                }));
                Ok(())
            }
            Tail::Open(_) => Err("extension listener already announced a tail".to_owned()),
            Tail::Closed => Err("extension listener prefix ended".to_owned()),
        }
    }

    /// Closes the listener prefix before returning its tail to the native bus.
    fn take_tail(&self) -> Result<Option<maestro_extensions::EventTail<Local>>, String> {
        match std::mem::replace(&mut *self.tail.borrow_mut(), Tail::Closed) {
            Tail::Open(tail) => Ok(tail),
            Tail::Closed => Err("extension listener prefix ended".to_owned()),
        }
    }
}
