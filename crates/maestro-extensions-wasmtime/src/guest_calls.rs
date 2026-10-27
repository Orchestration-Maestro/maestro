//! Retained guest callbacks submit actual exports to their original Store.
use crate::lifecycle::{
    StoreEndpoint,
    replies::{Replies, Reply},
};
use crate::{
    local_resources::{Admission, Registry},
    replaced_reads, writer_reads,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Weak},
};
use tokio::sync::mpsc::{UnboundedSender, WeakUnboundedSender};
use wasmtime::component::{ComponentType, Lower};

pub(crate) mod bash;
pub(crate) mod custom;
pub(crate) mod edit;
pub(crate) mod editors;
pub(crate) mod events;
pub(crate) mod factories;
pub(crate) mod find;
pub(crate) mod grep;
pub(crate) mod ls;
pub(crate) mod paints;
pub(crate) mod providers;
pub(crate) mod read;
pub(crate) mod widget_calls;
pub(crate) mod write;
pub(crate) use events::EventCall;
pub(crate) mod credentials;
pub(crate) mod registrations;
pub(crate) mod renderers;
pub(crate) mod schema;
pub(crate) mod terminal;
pub(crate) mod tool_renders;
pub(crate) mod tools;
pub(crate) use bash::{BashCall, BashRequest};
pub(crate) use registrations::RegisteredCall;
pub(crate) use tools::ToolCall;

/// A host-owned identity, independent of resource-table slot reuse.
pub(crate) struct Callback {
    /// Stable key used by the authored closure table.
    pub(crate) id: u32,
    /// An enumerated foreign node retained by the existing native registry.
    pub(crate) schema: Option<crate::local_resources::schema::Endpoint>,
}

/// Invocations consumed by the single Store driver, not by a replacement executor.
pub(crate) enum Call<H = Handle> {
    /// Invokes the actual Local message renderer.
    Renderer(renderers::RendererCall<H>),
    /// Calls a retained guest component through the same Store.
    Component(renderers::ComponentCall<H>),
    /// Enters the authored factory once under the same lifecycle as later callbacks.
    Start(Reply<()>),
    /// Marks a started invocation complete before native settlement.
    Finished,
    /// Invokes a concrete native event callback.
    Event(EventCall<H>),
    /// Executes one independently retained guest shell operation.
    Bash(BashCall<H>),
    /// Invokes one retained guest file observation.
    Read(read::Invocation<H>),
    /// Invokes one retained guest file effect.
    Write(write::Invocation<H>),
    /// Invokes one retained guest content search observation.
    Grep(grep::Invocation<H>),
    /// Invokes one retained guest file search observation.
    Find(find::Invocation<H>),
    /// Invokes retained directory observations.
    Ls(ls::Invocation<H>),
    /// Invokes one retained guest edit effect.
    Edit(edit::Invocation<H>),
    /// Invokes setup with the original writable session.
    Setup(H, writer_reads::Endpoint, Reply<()>),
    /// Invokes a continuation with the actual replacement context.
    WithSession(H, replaced_reads::Endpoint, Reply<()>),
    /// Invokes a registered command, completion or shortcut with original operands.
    Registered(RegisteredCall<H>),
    /// Invokes the real native tool registration callbacks.
    Tool(ToolCall<H>),
    /// Invokes the original OAuth interaction callback.
    OAuth(crate::oauth_callbacks::Invocation<H>),
    /// Calls an authored credential backend with its native scoped update.
    Credential(credentials::CredentialCall<H>),
    /// The original weak schema allocation and guest callback admission.
    Schema(schema::Invocation<H>),
    /// Delivers one chunk of terminal input to an authored listener.
    TerminalInput(terminal::Input<H>),
    /// Builds one component with the terminal and theme the native owner lent.
    Factory(factories::FactoryCall<H>),
    /// Builds a custom component or awaits its owned continuation.
    Custom(custom::Invocation<H>),
    /// Renders one tool row through its authored renderer.
    ToolRender(tool_renders::ToolRenderCall<H>),
    /// Calls an authored styling or highlighting function for a host widget.
    Paint(paints::PaintCall<H>),
    /// Calls an authored handler, style, truncation or submenu of a host list or loader.
    Widget(widget_calls::WidgetCall<H>),
    /// Builds one editor with what the native owner lent.
    EditorFactory(editors::FactoryCall<H>),
    /// Runs one native request on an authored editor.
    Editor(editors::EditorCall<H>),
    /// Runs a completion provider factory on the lent current provider.
    AutocompleteFactory(providers::FactoryCall<H>),
    /// Runs one native request on an authored completion provider.
    Autocomplete(providers::ProviderCall<H>),
    /// Awaits an authored hook on the original pending native model invocation.
    ModelHook(crate::model_calls::Invocation<H>),
    /// Sets up the original registered guest provider.
    Provider(crate::provider_calls::Invocation<H>),
    /// Operates on the original returned authored stream.
    Assistant(crate::provider_calls::Assistant<H>),
    /// Invokes an authored stream callback on the retained local event.
    Stream(crate::stream_calls::Invocation<H>),
    /// Enters one synchronous prefix through the active Store accessor.
    Synchronous(Synchronous<H>),
    /// Awaits the actual guest tail under its independently retained admission.
    Tail(H, Reply<()>),
    /// Releases the authored closure after its final native consumer.
    Release(Release),
}

/// Weak Store-side registration service; neither it nor its cache grants admission.
pub(crate) struct Callbacks {
    /// Next authored identity; table slots may be reused but identities are not.
    next: u32,
    /// One keyed fault and settlement owner for this Store.
    replies: Replies,
    /// Native callbacks, not Store state, retain the invocation sender.
    calls: WeakUnboundedSender<Call>,
    /// Aliases share final-release ownership without a Store-to-callback cycle.
    retained: BTreeMap<u32, Weak<Retained>>,
}

impl Callbacks {
    /// Connects callback registration to its original Store receiver.
    pub(crate) fn new(calls: WeakUnboundedSender<Call>, replies: Replies) -> Self {
        Self {
            next: 0,
            replies,
            calls,
            retained: BTreeMap::new(),
        }
    }

    /// Releases only the retired ownership generation, never a later native retention.
    ///
    /// An observer's release names no generation; it releases an identity no native alias
    /// retains.
    pub(crate) fn release(&mut self, release: &Release) -> Option<u32> {
        let retired = match self.retained.get(&release.id) {
            Some(owner) => owner.ptr_eq(&release.owner) && owner.strong_count() == 0,
            None => release.owner.ptr_eq(&Weak::new()),
        };
        if retired {
            self.retained.remove(&release.id);
            Some(release.id)
        } else {
            None
        }
    }

    /// Keeps a callback for an abort observer without retaining this Store.
    pub(crate) fn observer(&self, id: u32) -> Observer {
        Observer {
            id,
            calls: self.calls.clone(),
            replies: self.replies.clone(),
        }
    }

    /// Allocates a stable authored identity without keeping the Store alive.
    pub(crate) fn allocate(&mut self) -> Result<Callback, String> {
        let id = self.next;
        self.next = id
            .checked_add(1)
            .ok_or_else(|| "extension callback identity exhausted".to_owned())?;
        Ok(Callback { id, schema: None })
    }

    /// Retains one callback admission, shared by every native alias of this identity.
    pub(crate) fn retain(&mut self, id: u32, admission: Admission) -> Result<Handle, String> {
        self.retained_as(id, admission, false)
    }

    /// Retains a returned authored object, whose destruction waits for Store quiescence.
    pub(crate) fn retain_object(
        &mut self,
        id: u32,
        admission: Admission,
    ) -> Result<Handle, String> {
        self.retained_as(id, admission, true)
    }

    /// Shares one final-release owner per identity and records how it is destroyed.
    fn retained_as(
        &mut self,
        id: u32,
        admission: Admission,
        object: bool,
    ) -> Result<Handle, String> {
        if let Some(retained) = self.retained.get(&id).and_then(Weak::upgrade) {
            return Ok(Handle(retained));
        }
        let calls = self
            .calls
            .upgrade()
            .filter(|calls| !calls.is_closed())
            .ok_or_else(|| "extension Store owner ended".to_owned())?;
        let retained = Arc::new_cyclic(|owner| Retained {
            id,
            object,
            owner: owner.clone(),
            endpoint: StoreEndpoint::new(calls, admission, self.replies.clone()),
        });
        self.retained.insert(id, Arc::downgrade(&retained));
        Ok(Handle(retained))
    }
}

/// A queued final release names its native ownership generation as well as its identity.
pub(crate) struct Release {
    /// Queued final release retains admission through its actual guest invocation.
    pub(crate) calls: Option<UnboundedSender<Call>>,
    /// Healthy releases share the same tracked completion as native invocations.
    pub(crate) completion: Option<Reply<()>>,
    /// Authored closure identity.
    id: u32,
    /// An authored object is destroyed only while no other call is active.
    pub(crate) object: bool,
    /// A stale queued release cannot release a newly retained callback with the same id.
    owner: Weak<Retained>,
}

/// One final-release owner shared by copied native registrations and admitted calls.
struct Retained {
    /// Authored callback identity, not a Store resource-table key.
    id: u32,
    /// Whether the identity holds a returned authored object rather than a closure.
    object: bool,
    /// Weak self-identity permits final-release validation without retaining an owner.
    owner: Weak<Retained>,
    /// One lifecycle endpoint retains Store and native request admissions.
    endpoint: StoreEndpoint,
}

impl Drop for Retained {
    fn drop(&mut self) {
        let _ = self.endpoint.send(Call::Release(Release {
            calls: Some(self.endpoint.calls.clone()),
            completion: self
                .endpoint
                .replies
                .register(|_: Result<(), String>| {})
                .ok(),
            id: self.id,
            object: self.object,
            owner: self.owner.clone(),
        }));
    }
}

/// A callback kept by an abort observer, which outlives no Store: the signal it observes
/// may outlive the extension, so the observer holds the Store receiver weakly.
pub(crate) struct Observer {
    /// Authored callback identity.
    id: u32,
    /// Upgraded only for one invocation or the final release.
    calls: WeakUnboundedSender<Call>,
    /// The Store's fault and settlement owner.
    replies: Replies,
}

impl Observer {
    /// A callback alias for one invocation while the Store and `admission` are live.
    ///
    /// Its own release names its generation, which no table retains, so the closure stays.
    pub(crate) fn handle(&self, admission: Admission) -> Option<Handle> {
        let calls = self.calls.upgrade().filter(|calls| !calls.is_closed())?;
        Some(Handle(Arc::new_cyclic(|owner| Retained {
            id: self.id,
            object: false,
            owner: owner.clone(),
            endpoint: StoreEndpoint::new(calls, admission, self.replies.clone()),
        })))
    }
}

impl Drop for Observer {
    fn drop(&mut self) {
        if let Some(calls) = self.calls.upgrade() {
            let _ = calls.send(Call::Release(Release {
                calls: Some(calls.clone()),
                completion: self.replies.register(|_: Result<(), String>| {}).ok(),
                id: self.id,
                object: false,
                owner: Weak::new(),
            }));
        }
    }
}

/// Store-safe retained callback ownership; never contains a local domain context.
#[derive(Clone)]
pub(crate) struct Handle(Arc<Retained>);
#[derive(Clone)]
/// A non-admitting reference to an existing retained callback.
pub(crate) struct WeakHandle(Weak<Retained>);
impl WeakHandle {
    /// Recover a live original admission without manufacturing a callback.
    pub(crate) fn upgrade(&self) -> Option<Handle> {
        self.0.upgrade().map(Handle)
    }
}
impl Handle {
    /// Observe callback lifetime without keeping the Store alive.
    pub(crate) fn downgrade(&self) -> WeakHandle {
        WeakHandle(Arc::downgrade(&self.0))
    }
}

impl Handle {
    /// Returns the original authored identity for a temporary canonical borrow.
    pub(crate) fn id(&self) -> u32 {
        self.0.id
    }

    /// Returns the original shared lifecycle endpoint of this callback.
    pub(crate) fn endpoint(&self) -> &StoreEndpoint {
        &self.0.endpoint
    }

    /// Calls the actual setup export with the original writable owner.
    pub(crate) async fn setup(
        &self,
        registry: &Registry,
        session: writer_reads::Endpoint,
    ) -> Result<(), String> {
        self.0
            .endpoint
            .call(registry, |reply| Call::Setup(self.clone(), session, reply))
            .await
    }

    /// Calls the actual continuation export with its original replacement owner.
    pub(crate) async fn with_session(
        &self,
        registry: &Registry,
        context: replaced_reads::Endpoint,
    ) -> Result<(), String> {
        self.0
            .endpoint
            .call(registry, |reply| {
                Call::WithSession(self.clone(), context, reply)
            })
            .await
    }

    /// Adapts setup at the local boundary without copying or reconstructing the session.
    pub(crate) fn setup_callback(&self, registry: Registry) -> Setup {
        let callback = self.clone();
        std::rc::Rc::new(move |session| {
            let endpoint = registry.writable(session);
            let (callback, registry) = (callback.clone(), registry.clone());
            Box::pin(async move {
                callback
                    .setup(&registry, endpoint?)
                    .await
                    .map_err(Into::into)
            })
        })
    }

    /// Adapts a continuation without selecting a later current context.
    pub(crate) fn continuation(&self, registry: Registry) -> Continuation {
        let callback = self.clone();
        std::rc::Rc::new(move |context| {
            let endpoint = registry.replaced(context);
            let (callback, registry) = (callback.clone(), registry.clone());
            Box::pin(async move {
                callback
                    .with_session(&registry, endpoint?)
                    .await
                    .map_err(Into::into)
            })
        })
    }
}

/// Native setup signature owned by the command-context domain.
type Setup = std::rc::Rc<
    dyn Fn(std::rc::Rc<std::cell::RefCell<maestro_extensions::SessionManager>>) -> NativeFuture,
>;
/// Native continuation signature owned by the command-context domain.
type Continuation = std::rc::Rc<dyn Fn(maestro_extensions::ReplacedSessionContext) -> NativeFuture>;
/// Local callback future, driven by the existing command owner.
type NativeFuture = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<(), maestro_extensions::EventError>>>,
>;

/// Actual synchronous guest entry selected by the native callback owner.
#[derive(ComponentType, Lower, Clone, Copy)]
#[component(enum)]
#[repr(u8)]
pub(crate) enum SynchronousKind {
    /// Native event listener prefix.
    #[component(name = "listener")]
    Listener,
    /// Native compaction completion.
    #[component(name = "compaction-complete")]
    CompactionComplete,
    /// Native compaction failure.
    #[component(name = "compaction-error")]
    CompactionError,
    /// Native abort observer; its export takes no data.
    #[component(name = "abort")]
    Abort,
    /// Native component disposal; its export takes no data.
    #[component(name = "dispose")]
    Dispose,
    /// Native branch change of a footer's metadata; its export takes no data.
    #[component(name = "branch-change")]
    BranchChange,
    /// Native file notification; its export takes no data.
    #[component(name = "watch")]
    Watch,
}

/// Owned canonical operands of one synchronous callback invocation.
pub(crate) struct Synchronous<H = Handle> {
    /// Retains the original authored callback and its admission.
    pub(crate) handler: H,
    /// Selects the actual canonical guest export.
    pub(crate) kind: SynchronousKind,
    /// Exact encoded value or unmodified error message.
    pub(crate) data: String,
    /// Original native invocation frame, not an ordinary-bus substitute.
    pub(crate) emitter: crate::local_resources::effects::EmitterEndpoint,
    /// Returns through the same local request receiver.
    pub(crate) completion: Reply<()>,
}

impl Handle {
    /// Returns the guest's tail to the original native bus for its normal settlement.
    pub(crate) fn listener(
        &self,
        registry: &Registry,
        data: &serde_json::Value,
        frame: maestro_extensions::callbacks::CallbackFrame<maestro_extensions::Local>,
    ) -> Result<Option<maestro_extensions::EventTail<maestro_extensions::Local>>, String> {
        let emitter = registry.emitter(frame)?;
        let result = self.synchronous(
            registry,
            SynchronousKind::Listener,
            data.to_string(),
            emitter.clone(),
        );
        let tail = registry.take_tail(&emitter)?;
        result.map(|()| tail)
    }

    /// Services native reads during a synchronous active-Accessor guest entry.
    pub(crate) fn synchronous(
        &self,
        registry: &Registry,
        kind: SynchronousKind,
        data: String,
        emitter: crate::local_resources::effects::EmitterEndpoint,
    ) -> Result<(), String> {
        self.0.endpoint.synchronous(registry, |completion| {
            Ok(Call::Synchronous(Synchronous {
                handler: self.clone(),
                kind,
                data,
                emitter,
                completion,
            }))
        })
    }

    /// Runs the tail only when the native bus polls its original queued work.
    pub(crate) async fn tail(&self, registry: &Registry) -> Result<(), String> {
        self.0
            .endpoint
            .call(registry, |reply| Call::Tail(self.clone(), reply))
            .await
    }
}
