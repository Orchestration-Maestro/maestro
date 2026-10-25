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
pub(crate) mod events;
pub(crate) use events::EventCall;
pub(crate) mod registrations;
pub(crate) mod renderers;
pub(crate) mod tools;
pub(crate) use bash::{BashCall, BashRequest};
pub(crate) use registrations::RegisteredCall;
pub(crate) use tools::ToolCall;

/// A host-owned identity, independent of resource-table slot reuse.
pub(crate) struct Callback {
    /// Stable key used by the authored closure table.
    pub(crate) id: u32,
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
    /// Invokes setup with the original writable session.
    Setup(H, writer_reads::Endpoint, Reply<()>),
    /// Invokes a continuation with the actual replacement context.
    WithSession(H, replaced_reads::Endpoint, Reply<()>),
    /// Invokes a registered command, completion or shortcut with original operands.
    Registered(RegisteredCall<H>),
    /// Invokes the real native tool registration callbacks.
    Tool(ToolCall<H>),
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
    pub(crate) fn release(&mut self, release: &Release) -> Option<u32> {
        let retired = self
            .retained
            .get(&release.id)
            .is_some_and(|owner| owner.ptr_eq(&release.owner) && owner.strong_count() == 0);
        if retired {
            self.retained.remove(&release.id);
            Some(release.id)
        } else {
            None
        }
    }

    /// Allocates a stable authored identity without keeping the Store alive.
    pub(crate) fn allocate(&mut self) -> Result<Callback, String> {
        let id = self.next;
        self.next = id
            .checked_add(1)
            .ok_or_else(|| "extension callback identity exhausted".to_owned())?;
        Ok(Callback { id })
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

/// Store-safe retained callback ownership; never contains a local domain context.
#[derive(Clone)]
pub(crate) struct Handle(Arc<Retained>);

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
