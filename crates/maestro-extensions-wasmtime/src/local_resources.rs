//! One local request pump with independently retained domain capabilities.
use crate::context_reads::{Endpoint, Owner, Request};
use maestro_extensions::{
    Cancellation, ExtensionUIContext, Model, ModelRegistry, ReadonlySessionManager,
    ResolvedRequestAuth, SharedHandle,
};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc};
use tokio::sync::mpsc::{UnboundedSender, WeakUnboundedSender};

pub(crate) mod bash_output;
pub(crate) mod builtin;
pub(crate) mod builtin_bash;
/// Exact-name native selection with independently retained effects.
pub(crate) mod builtin_selection;
pub(crate) mod credential_scope;
mod dialogs;
pub(crate) mod effects;
mod footer;
pub(crate) mod host_operations;
pub(crate) mod http;
pub(crate) mod local_bash;
pub(crate) mod model_streams;
pub(crate) mod native_io;
pub(crate) mod native_resources;
mod overlays;
pub(crate) mod provider_controls;
mod providers;
mod rendering;
pub(crate) mod schema;
mod signals;
pub(crate) mod stream_reads;
pub(crate) mod synchronous;
pub(crate) mod watches;
mod widgets;
pub(crate) use synchronous::Completion;

/// Local admitted work never moves to the component's Store thread.
pub(crate) type Work = std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>;

/// Typed requests and final resource releases arriving from the Store thread.
pub(crate) enum Message {
    /// Completes an admitted synchronous guest call through this same receiver.
    Completed(u64, Result<(), String>),
    /// Calls a synchronous update still borrowed on its native caller's stack.
    LockUpdate(
        u64,
        Option<String>,
        std::sync::mpsc::Sender<Result<Option<String>, String>>,
    ),
    /// Calls a Send update still borrowed by its native pending future.
    AsyncLockUpdate(u64, credential_scope::AsyncRequest),
    /// Retires the async operand only when its actual native future ends.
    EndLockUpdate(u64),
    /// The original retained Json.
    Json(u64, crate::schema_reads::Request),
    /// Live declarations with their original schema callback identities.
    SchemaListing(
        u64,
        std::sync::mpsc::Sender<Result<crate::schema_registration::ToolListing, String>>,
    ),
    /// Immediately resolve and invoke the registered native model adapter.
    ModelStart(model_streams::Creation),
    /// Routes registered-provider setup and native hook calls.
    Provider(provider_controls::Command),
    /// Operate on its original assistant FIFO and independent result.
    ModelStream(u64, model_streams::Request),
    /// Construct the models-owned buffer with retained local callbacks.
    NewStream(stream_reads::Creation),
    /// Operate on the original native event buffer.
    Stream(u64, crate::local_resources::stream_reads::Request),
    /// An operation on its original guarded context.
    Read(u64, Request),
    /// Uses the original API's registration and runtime action owners.
    Api(u64, crate::api_reads::Request),
    /// Reads or prepares an original native built-in definition.
    Builtin(u64, crate::builtin_reads::Request),
    /// Constructs, executes or renders through an original native built-in owner.
    BuiltinOperation(builtin::Operation),
    /// Registers retained callbacks through the original facade.
    Registration(u64, crate::registration_reads::Request),
    /// Uses the original facade bus for listener registration and emission.
    Events(u64, crate::event_reads::Request),
    /// Selects and admits a model operation without blocking synchronous reads.
    ApiModel(
        u64,
        Box<Model>,
        tokio::sync::oneshot::Sender<Result<bool, String>>,
        Admission,
    ),
    /// Starts explicit execution through its original facade and signal.
    Exec(
        u64,
        crate::exec::Request,
        tokio::sync::oneshot::Sender<Result<String, String>>,
        Admission,
    ),
    /// Calls the original factory-time settings and package owner.
    HostOperation(host_operations::Call),
    /// Waits through the native owner without blocking other requests.
    Sleep(
        native_io::Wait,
        tokio::sync::oneshot::Sender<Result<(), String>>,
        Admission,
    ),
    /// Starts an admitted command operation without blocking request consumption.
    Command(u64, crate::command_reads::Request, Admission),
    /// Creates one independently owned cancellation on this local owner.
    NewSignal(
        Option<Cancellation>,
        std::sync::mpsc::Sender<Result<SignalEndpoint, String>>,
    ),
    /// Uses the original retained cancellation, not the current context signal.
    Signal(u64, signals::Request),
    /// Reads the original live history independently of guarded contexts.
    History(u64, crate::history_reads::Request),
    /// Mutates the original writable session supplied to setup.
    Writer(u64, crate::writer_reads::Request),
    /// Acquires an independent live reader from the writable setup capability.
    WriterReader(
        u64,
        std::sync::mpsc::Sender<Result<crate::history_reads::Endpoint, String>>,
    ),
    /// Runs the retained provider's native authorization with original interaction.
    OAuthLogin(
        u64,
        crate::oauth_callbacks::Controls,
        tokio::sync::oneshot::Sender<Result<String, String>>,
        Admission,
    ),
    /// Awaits token rotation while retaining the originally selected provider.
    OAuthRefresh(
        u64,
        String,
        tokio::sync::oneshot::Sender<Result<String, String>>,
        Admission,
    ),
    /// Uses native interaction callbacks on this same local owner.
    OAuthInteraction(crate::oauth_interaction::Request),
    /// Calls an independently retained OAuth implementation.
    OAuthProvider(u64, crate::oauth_reads::Request),
    /// Uses independently retained accepted credentials.
    Credentials(u64, crate::credential_reads::Request),
    /// Authorizes and persists through the retained credential owner.
    CredentialLogin(
        u64,
        String,
        crate::oauth_callbacks::Controls,
        tokio::sync::oneshot::Sender<Result<(), String>>,
        Admission,
    ),
    /// Resolves a key while retaining the original owner through settlement.
    CredentialKey(
        u64,
        (String, Option<bool>),
        tokio::sync::oneshot::Sender<Result<Option<String>, String>>,
        Admission,
    ),
    /// Reads the original live catalog, not a captured descriptor list.
    Catalog(u64, crate::catalog_reads::Request),
    /// Resolves credentials using the caller's effects without blocking other requests.
    Auth(
        u64,
        Box<Model>,
        tokio::sync::oneshot::Sender<Result<ResolvedRequestAuth, String>>,
        Admission,
    ),
    /// Resolves a key through the original catalog's credential and config owners.
    ProviderKey(
        u64,
        String,
        tokio::sync::oneshot::Sender<Result<Option<String>, String>>,
        Admission,
    ),
    /// Reads one retained node of an already-built tree.
    Node(u64, crate::tree_reads::Request),
    /// Sends through the original creating session, even after guarded reads retire.
    Replaced(u64, crate::replaced_reads::Request, Admission),
    /// Retains the replacement's original guarded command view independently.
    ReplacedCommand(u64, std::sync::mpsc::Sender<Result<Endpoint, String>>),
    /// Uses original native observers and explicit subscription removers.
    Effect(u64, effects::Request),
    /// Runs a request whose native code may call back into this instance through its route.
    Routed(
        Box<Message>,
        std::sync::mpsc::Sender<crate::import_route::Nested>,
    ),
    /// Forwards an owned chunk into its original execution's data stream.
    BashOutput(u64, Vec<u8>, bash_output::Reply),
    /// Uses the originally acquired UI independently of later bindings.
    Ui(u64, crate::ui_reads::Request),
    /// Admits one dialog on the originally acquired UI.
    Dialog(u64, crate::ui_reads::DialogRequest, Admission),
    /// Reads or edits the independently retained live renderer theme.
    Theme(u64, crate::theme_reads::Request),
    /// Reads or subscribes to the footer metadata lent to a footer factory.
    Footer(u64, crate::footer_data::Request),
    /// Builds, uses or lends host widgets and styles.
    Widgets(crate::widget_reads::WidgetMessage),
    /// Typed operations on original application-supplied native resources.
    Native(u64, native_resources::Request),
    /// HTTP operations retain their native response and signal.
    Http(u64, http::Request),
    /// Native watch registration and explicit close on the same owner.
    Watch(u64, watches::Request),
    /// Final release of an independently retained capability.
    Release(u64),
}

/// A startup or callback admission keeping the pump available.
#[derive(Clone)]
pub(crate) struct Admission {
    /// Closing the final strong sender ends the local pump.
    _requests: UnboundedSender<Message>,
}

/// The Store's shared ownership of one local resource identity.
pub(crate) struct Lease {
    /// Identity unique within this extension's resource registry.
    id: u64,
    /// Requests keep the owner alive until the final resource consumer drops.
    requests: UnboundedSender<Message>,
}

impl Lease {
    /// Provides the original Send endpoint for pump-thread credential callbacks.
    pub(crate) fn requests(&self) -> &UnboundedSender<Message> {
        &self.requests
    }
    /// Retains this original owner's admission for a native callback or its work.
    pub(crate) fn admission(&self) -> Admission {
        Admission {
            _requests: self.requests.clone(),
        }
    }

    /// Admits asynchronous work before sending it to the same local receiver.
    pub(crate) async fn request_async<T>(
        &self,
        request: impl FnOnce(u64, tokio::sync::oneshot::Sender<Result<T, String>>, Admission) -> Message,
    ) -> Result<T, String> {
        let (reply, response) = tokio::sync::oneshot::channel();
        self.requests
            .send(request(self.id, reply, self.admission()))
            .map_err(|_| "extension resource owner ended".to_owned())?;
        response
            .await
            .map_err(|_| "extension resource reply ended".to_owned())?
    }

    /// Sends a typed request and waits only on the dedicated Store thread.
    pub(crate) fn request<T>(
        &self,
        request: impl FnOnce(u64, std::sync::mpsc::Sender<Result<T, String>>) -> Message,
    ) -> Result<T, String> {
        let (reply, response) = std::sync::mpsc::channel();
        self.requests
            .send(request(self.id, reply))
            .map_err(|_| "extension resource owner ended".to_owned())?;
        response
            .recv()
            .map_err(|_| "extension resource reply ended".to_owned())?
    }

    /// Sends a request whose native code may call back into this instance; the guest
    /// drives the returned route until the request's own result arrives.
    pub(crate) fn route<T: Into<crate::import_route::Value> + Send + 'static>(
        &self,
        request: impl FnOnce(u64, std::sync::mpsc::Sender<Result<T, String>>) -> Message,
    ) -> crate::import_route::Route {
        let (reply, response) = std::sync::mpsc::channel();
        let (nested, route) = crate::import_route::Route::new(response);
        let _ = self
            .requests
            .send(Message::Routed(Box::new(request(self.id, reply)), nested));
        route
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        let _ = self.requests.send(Message::Release(self.id));
    }
}

pub(crate) use crate::ui_reads::Endpoint as UiEndpoint;

pub(crate) use signals::SignalEndpoint;

/// Independently owned capabilities, never sent to the Store.
enum Capability {
    /// Independently retained OAuth implementation.
    OAuthProvider(maestro_extensions::OAuthProviderHandle),
    /// Original native options and arbitrary selected author signal bindings.
    ProviderControls(Box<provider_controls::Owner>, crate::model_calls::Scopes),
    /// Original native notification registration, closed on its final release.
    NativeWatcher(Rc<dyn maestro_extensions::native_io::FsWatcher>),
    /// Original native child, independently retained after its creating callback.
    NativeChild(Rc<dyn maestro_extensions::native_io::ChildProcess>),
    /// Original native response, never a body snapshot.
    NativeResponse(Rc<dyn maestro_extensions::native_io::Response>),
    /// Original native consuming output.
    NativeReader(Rc<dyn maestro_extensions::native_io::Readable>),
    /// Original native ordered input.
    NativeWriter(Rc<dyn maestro_extensions::native_io::Writable>),
    /// An independently retained enumerated schema node.
    Schema(maestro_extensions::SchemaNode),
    /// Original native model invocation buffer.
    AssistantStream(maestro_extensions::AssistantMessageEventStream),
    /// Native event buffer with caller-local completion callbacks.
    Stream(crate::local_resources::stream_reads::Owner),
    /// Original typed native definition behind the component's tool resource.
    Builtin(Rc<dyn crate::builtin_tools::Definition>),
    /// Original native local backend, retained independently of a returned guest operation.
    LocalBash(Rc<dyn maestro_extensions::BashOperations<maestro_extensions::Local>>),
    /// Original row context, including its environment and shared state.
    NativeRow(crate::native_renders::Row),
    /// Original native component, not a guest proxy.
    NativeComponent(maestro_extensions::ComponentHandle),
    /// Original guarded base or command context.
    Context(Owner),
    /// Original registration facade; its domain owner remains weakly referenced.
    Api(maestro_extensions::ExtensionAPI<maestro_extensions::Local>),
    /// Original session's continuation capability, including its own message routes.
    Replaced(maestro_extensions::ReplacedSessionContext),
    /// Original cancellation, unaffected by context retirement or rebinding.
    Signal(Cancellation),
    /// Original read-only session, never a frozen serialized snapshot.
    History(Rc<RefCell<dyn ReadonlySessionManager>>),
    /// Original writable setup capability, not a copied or reconstructed history.
    Writer(Rc<RefCell<maestro_extensions::SessionManager>>),
    /// Original credentials and their request-time effects.
    Credentials(
        Arc<maestro_extensions::AuthStorage>,
        maestro_extensions::ConfigValues,
    ),
    /// Original independently retained catalog.
    Catalog(SharedHandle<ModelRegistry>),
    /// A captured node, independent of the reader that produced its tree.
    Node(crate::tree_reads::Owner),
    /// Original observer supplied by native tool invocation.
    ToolUpdate(
        maestro_extensions::AgentToolUpdateCallback<serde_json::Value, maestro_extensions::Local>,
    ),
    /// Original callback invocation's bus and deferred queue, with native retirement.
    CallbackFrame(effects::Frame),
    /// Owned data stream for one execution; no borrowed sink is retained.
    BashOutput(Rc<bash_output::Frame>),
    /// Original explicit remover from the existing event bus.
    Subscription(maestro_extensions::Subscription<maestro_extensions::Local>),
    /// Removes an abort observer explicitly; dropping it keeps the observer.
    SignalSubscription(Rc<dyn Fn()>),
    /// Original live theme supplied by native rendering.
    Theme(maestro_extensions::LiveTheme),
    /// Removes a terminal input listener from the UI it was added to; dropping it keeps the
    /// listener.
    UiSubscription(Rc<dyn Fn() -> Result<(), maestro_extensions::EventError>>),
    /// Original raw UI capability, independent of later bindings.
    Ui(ExtensionUIContext),
    /// Original footer metadata lent to a footer factory.
    Footer(Rc<dyn maestro_extensions::footer_data_provider::ReadonlyFooterDataProvider>),
    /// Original redraw capability of one tool row.
    Invalidate(Rc<dyn Fn()>),
    /// A host widget the extension built.
    Widget(Rc<crate::widget_reads::Widget>),
    /// A lent host style.
    Style(maestro_extensions::widgets::BorderColor),
    /// The lent host highlighting.
    Highlighter(Rc<crate::guest_calls::paints::HighlightFn>),
    /// Shared native overlay placement and its bound error reporter.
    OverlayOptions(overlays::Placement),
    /// A shown overlay's native handle, shared with the host.
    OverlayHandle(overlays::SharedHandle),
    /// Original custom-component close operation.
    CustomClose(maestro_extensions::types::CustomClose),
    /// Original terminal lent to a component factory, with the reporter of failures of the
    /// callbacks it runs.
    Tui(
        maestro_extensions::TUI,
        Option<maestro_extensions::types::UIErrorReporter>,
    ),
    /// A lent settings row style.
    RowStyle(Rc<crate::widget_lists::RowStyleFn>),
    /// Closes an open settings submenu.
    SubmenuDone(Rc<dyn Fn(Option<String>)>),
    /// A keybindings manager.
    Keybindings(maestro_extensions::widgets::ExtensionKeybindings),
    /// A native text callback lent to an authored editor.
    Text(maestro_extensions::widgets::TextCallback),
    /// A native completion provider lent to the guest.
    Provider(crate::guest_calls::providers::Native),
    /// An editor factory another extension set.
    EditorFactory(maestro_extensions::types::EditorFactory),
}

/// Local entries and their monotonic identities.
#[derive(Default)]
pub(crate) struct Resources {
    /// Native effects supplied by the application's composition, never reconstructed here.
    pub(crate) native_io: Option<Rc<dyn maestro_extensions::native_io::NativeIo>>,
    /// Next identity; released identities are never reused.
    next: u64,
    /// Weak native callback identities by allocation address; each weak handle keeps its
    /// address from reuse and retains no guest admission.
    pub(crate) identities: std::collections::HashMap<usize, Identity>,
    /// Identity count at which retired identities are dropped, doubling so the work stays linear.
    prune_at: usize,
    /// Each entry owns exactly the capability identified by its lease.
    entries: BTreeMap<u64, Rc<Capability>>,
}

/// Local insertion service; its weak sender cannot keep the pump alive.
#[derive(Clone)]
pub(crate) struct Registry {
    /// Shared local entries; never borrowed while invoking domain code.
    pub(crate) resources: Rc<RefCell<Resources>>,
    /// Registration services do not grant their own admission.
    requests: WeakUnboundedSender<Message>,
    /// Receiver access is weak until a synchronous invocation has been admitted.
    pump: std::rc::Weak<pump::State>,
    /// Builds the widgets whose look depends on the host.
    widgets: Rc<dyn maestro_extensions::ExtensionWidgets>,
}

impl Registry {
    /// Retains the actual implementation selected by native provider enumeration.
    pub(crate) fn oauth_provider(
        &self,
        provider: maestro_extensions::OAuthProviderHandle,
    ) -> Result<crate::oauth_reads::Endpoint, String> {
        self.insert(Capability::OAuthProvider(provider))
            .map(crate::oauth_reads::Endpoint)
    }

    /// Selects the retained credentials only when the supplied lease belongs to this pump.
    pub(crate) fn credential_owner(
        &self,
        endpoint: &crate::credential_reads::Endpoint,
    ) -> Result<
        (
            Arc<maestro_extensions::AuthStorage>,
            maestro_extensions::ConfigValues,
        ),
        String,
    > {
        if !self.owns(&endpoint.0) {
            return Err("extension credential resource belongs to another host".into());
        }
        match self.capability(endpoint.0.id).as_deref() {
            Some(Capability::Credentials(auth, effects)) => Ok((auth.clone(), effects.clone())),
            _ => Err("extension credential resource ended".into()),
        }
    }
    /// Checks provenance without relying on registry-local resource identities.
    pub(crate) fn owns(&self, lease: &Lease) -> bool {
        self.requests
            .upgrade()
            .is_some_and(|requests| requests.same_channel(&lease.requests))
    }
    /// Clones one original capability without holding a table borrow during native calls.
    fn capability(&self, id: u64) -> Option<Rc<Capability>> {
        self.resources.borrow().entries.get(&id).cloned()
    }

    /// Retains the supplied facade without constructing a replacement registration owner.
    pub(crate) fn api(
        &self,
        api: maestro_extensions::ExtensionAPI<maestro_extensions::Local>,
    ) -> Result<crate::api_reads::Endpoint, String> {
        self.insert(Capability::Api(api))
            .map(crate::api_reads::Endpoint)
    }

    /// Clones the original facade bus without selecting another extension identity.
    pub(crate) fn events(
        &self,
        api: &crate::api_reads::Endpoint,
    ) -> Result<maestro_extensions::EventBus<maestro_extensions::Local>, String> {
        self.facade(api).map(|api| api.events)
    }

    /// The original extension facade behind an API resource.
    pub(crate) fn facade(
        &self,
        api: &crate::api_reads::Endpoint,
    ) -> Result<maestro_extensions::ExtensionAPI<maestro_extensions::Local>, String> {
        let capability = self.capability(api.0.id);
        match capability.as_deref() {
            Some(Capability::Api(api)) => Ok(api.clone()),
            _ => Err("extension API resource ended".to_owned()),
        }
    }

    /// Retains the exact continuation context supplied by a session operation.
    pub(crate) fn replaced(
        &self,
        context: maestro_extensions::ReplacedSessionContext,
    ) -> Result<crate::replaced_reads::Endpoint, String> {
        self.insert(Capability::Replaced(context))
            .map(crate::replaced_reads::Endpoint)
    }

    /// Retains a context selected by the actual caller, not by the factory.
    pub(crate) fn context(&self, owner: Owner) -> Result<Endpoint, String> {
        self.insert(Capability::Context(owner)).map(Endpoint)
    }

    /// Retains the UI returned by the guarded read at consumption time.
    pub(crate) fn ui(&self, ui: ExtensionUIContext) -> Result<UiEndpoint, String> {
        self.insert(Capability::Ui(ui)).map(UiEndpoint)
    }

    /// Resolves an original cancellation without retaining the capability table borrow.
    fn cancellation(&self, signal: &SignalEndpoint) -> Result<Cancellation, String> {
        match self.capability(signal.0.id).as_deref() {
            Some(Capability::Signal(signal)) => Ok(signal.clone()),
            _ => Err("extension signal resource ended".to_owned()),
        }
    }

    /// Retains the cancellation selected by the context read.
    pub(crate) fn signal(&self, signal: Cancellation) -> Result<SignalEndpoint, String> {
        self.insert(Capability::Signal(signal)).map(SignalEndpoint)
    }

    /// Retains the exact history capability returned by the domain context.
    pub(crate) fn history(
        &self,
        history: Rc<RefCell<dyn ReadonlySessionManager>>,
    ) -> Result<crate::history_reads::Endpoint, String> {
        self.insert(Capability::History(history))
            .map(crate::history_reads::Endpoint)
    }

    /// Retains the exact session supplied to a setup callback.
    pub(crate) fn writable(
        &self,
        session: Rc<RefCell<maestro_extensions::SessionManager>>,
    ) -> Result<crate::writer_reads::Endpoint, String> {
        self.insert(Capability::Writer(session))
            .map(crate::writer_reads::Endpoint)
    }

    /// Retains the catalog selected by the guarded context read.
    pub(crate) fn catalog(
        &self,
        catalog: SharedHandle<ModelRegistry>,
    ) -> Result<crate::catalog_reads::Endpoint, String> {
        self.insert(Capability::Catalog(catalog))
            .map(crate::catalog_reads::Endpoint)
    }

    /// Retains accepted credentials together with their original request-time effects.
    pub(crate) fn credentials(
        &self,
        owner: Arc<maestro_extensions::AuthStorage>,
        effects: maestro_extensions::ConfigValues,
    ) -> Result<crate::credential_reads::Endpoint, String> {
        self.insert(Capability::Credentials(owner, effects))
            .map(crate::credential_reads::Endpoint)
    }

    /// Retains a captured node without keeping the guarded context or reader alive.
    pub(crate) fn node(
        &self,
        node: crate::tree_reads::Owner,
    ) -> Result<crate::tree_reads::Endpoint, String> {
        self.insert(Capability::Node(node))
            .map(crate::tree_reads::Endpoint)
    }

    /// Returns the innermost native operation that currently owns this instance's route.
    fn route(&self) -> Option<std::sync::mpsc::Sender<crate::import_route::Nested>> {
        self.pump
            .upgrade()
            .and_then(|pump| pump.routes.borrow().last().cloned())
    }

    /// Hands a synchronous target to the native operation that currently owns this instance,
    /// or submits it to the Store when no operation does.
    pub(crate) fn dispatch(
        &self,
        endpoint: &crate::lifecycle::StoreEndpoint,
        call: crate::guest_calls::Call,
    ) -> Result<(), String> {
        match self.route() {
            Some(route) => crate::import_route::Nested::send(&route, call, None),
            None => endpoint.send(call),
        }
    }

    /// Hands an asynchronous target to the native operation that owns this instance and
    /// waits until its prefix ran, or submits it to the Store when no operation does.
    pub(crate) fn admit(
        &self,
        endpoint: &crate::lifecycle::StoreEndpoint,
        call: crate::guest_calls::Call,
    ) -> Result<(), String> {
        match self.route() {
            Some(route) => self.invoke_sync(None, |prefix, _| {
                crate::import_route::Nested::send(&route, call, Some(prefix))
            }),
            None => endpoint.send(call),
        }
    }

    /// The pump's request sender while an existing admission keeps it live.
    pub(crate) fn requests(&self) -> Result<UnboundedSender<Message>, String> {
        self.requests
            .upgrade()
            .filter(|requests| !requests.is_closed())
            .ok_or_else(|| "extension resource owner ended".to_owned())
    }

    /// A further admission while an existing one keeps the pump live.
    pub(crate) fn admission(&self) -> Result<Admission, String> {
        self.requests().map(|requests| Admission {
            _requests: requests,
        })
    }

    /// Allocates one independent identity while an existing admission is live.
    fn insert(&self, capability: Capability) -> Result<Arc<Lease>, String> {
        let requests = self.requests()?;
        let id = {
            let mut resources = self.resources.borrow_mut();
            let id = resources.next;
            resources.next = id
                .checked_add(1)
                .ok_or_else(|| "extension resource identity exhausted".to_owned())?;
            id
        };
        let previous = self
            .resources
            .borrow_mut()
            .entries
            .insert(id, Rc::new(capability));
        drop(previous);
        Ok(Arc::new(Lease { id, requests }))
    }
}

pub(crate) use pump::Pump;
mod pump;

#[cfg(test)]
mod tests;

/// Weak callback identities share one cache without retaining domain code or Store admission.
pub(crate) enum Identity {
    /// The original weak listener allocation and guest callback identity.
    Listener(
        std::rc::Weak<
            <maestro_extensions::Local as maestro_extensions::ExtensionExecution>::Listener,
        >,
        u32,
    ),
    /// The original weak schema allocation and guest callback admission.
    Schema(crate::schema_registration::Identity),
    /// A native editor factory and the identity of the authored factory it runs.
    EditorFactory(
        maestro_extensions::types::WeakUIHostCall<
            (
                maestro_extensions::TUI,
                maestro_extensions::widgets::EditorTheme,
                maestro_extensions::widgets::ExtensionKeybindings,
            ),
            maestro_extensions::types::EditorFactoryOutcome,
        >,
        u32,
    ),
}
impl Identity {
    /// Whether the original callback still has a strong capture owner.
    pub(crate) fn alive(&self) -> bool {
        match self {
            Self::Listener(weak, _) => weak.strong_count() > 0,
            Self::Schema((weak, _)) => weak.strong_count() > 0,
            Self::EditorFactory(weak, _) => weak.alive(),
        }
    }
}
impl Resources {
    /// Insert one weak identity with amortized retirement.
    pub(crate) fn identity(&mut self, key: usize, identity: Identity) {
        if self.identities.len() >= self.prune_at {
            #[cfg(test)]
            effects::emission::CAPACITY
                .set(effects::emission::CAPACITY.get() + self.identities.capacity());
            self.identities.retain(|_, identity| {
                effects::emission::inspect();
                identity.alive()
            });
            // `retain` scans every bucket, so retired capacity must not outlive the prune.
            self.identities.shrink_to_fit();
            self.prune_at = 2 * self.identities.len() + 1;
        }
        self.identities.insert(key, identity);
    }
}
