//! Shared local ownership of the sole request receiver and admitted work.
use super::{Admission, Capability, Message, Registry, Work};
use crate::context_reads::Request;
use futures_util::{StreamExt, stream::FuturesUnordered};
use maestro_extensions::Cancellation;
use std::{
    cell::RefCell,
    rc::Rc,
    task::{Context, Poll},
};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

/// The sole request receiver for one extension, driven on its local owner.
pub(crate) struct Pump {
    /// Receiver ownership shared only with active local synchronous waits.
    state: Rc<State>,
}

/// Local state; weak registration services cannot keep it or its admission alive.
pub(super) struct State {
    /// Local capability ownership and weak insertion service.
    pub(super) registry: Registry,
    /// Reads and releases share one ordered request stream.
    requests: RefCell<UnboundedReceiver<Message>>,
    /// The one pending set is polled only between native admissions.
    pending: RefCell<FuturesUnordered<Work>>,
    /// First-polled futures awaiting insertion outside the set's current poll.
    staging: RefCell<Vec<Work>>,
    /// Current asynchronous or synchronous waiter, copied before native code runs.
    pub(super) waker: RefCell<std::task::Waker>,
    /// Typed completion storage for active synchronous guest invocations.
    pub(super) synchronous: super::synchronous::Returns,
    /// Native operations in progress, innermost last, whose nested targets their guest runs.
    pub(super) routes: RefCell<Vec<std::sync::mpsc::Sender<crate::import_route::Nested>>>,
    /// Dropped after local requests and pending work release their reply senders.
    worker: RefCell<Option<Worker>>,
}

/// The native pump owns joining its Store thread, including thread-local destruction.
struct Worker {
    /// Taken once when the pump retires.
    thread: Option<std::thread::JoinHandle<()>>,
    /// Wakes the Store without retaining its lifetime.
    shutdown: std::sync::Weak<tokio::sync::Notify>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.upgrade() {
            shutdown.notify_one();
        }
        let thread = JoinGuard(self.thread.take());
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            // Keeps the local executor running. Shutdown may drop the task unstarted; that joins too.
            drop(runtime.spawn_blocking(move || drop(thread)));
        } else {
            drop(thread);
        }
    }
}

/// Joins its thread when dropped, even when Tokio discards the blocking task holding it.
struct JoinGuard(Option<std::thread::JoinHandle<()>>);

impl Drop for JoinGuard {
    fn drop(&mut self) {
        if let Some(thread) = self.0.take() {
            let _ = thread.join();
        }
    }
}

impl Pump {
    /// Separates startup admission from the local owner without spawning work.
    pub(crate) fn new(widgets: Rc<dyn maestro_extensions::ExtensionWidgets>) -> (Self, Admission) {
        let (requests, receiver) = unbounded_channel();
        let state = Rc::new_cyclic(|state| State {
            registry: Registry {
                resources: Rc::default(),
                requests: requests.downgrade(),
                pump: state.clone(),
                widgets,
            },
            requests: RefCell::new(receiver),
            pending: RefCell::new(FuturesUnordered::new()),
            staging: RefCell::default(),
            waker: RefCell::new(std::task::Waker::noop().clone()),
            synchronous: super::synchronous::Returns::default(),
            routes: RefCell::default(),
            worker: RefCell::default(),
        });
        super::credential_scope::register(Rc::downgrade(&state));
        (
            Self { state },
            Admission {
                _requests: requests,
            },
        )
    }

    /// Retains the Store thread until retirement has disconnected native replies.
    pub(crate) fn own_worker(
        &self,
        thread: std::thread::JoinHandle<()>,
        shutdown: std::sync::Weak<tokio::sync::Notify>,
    ) {
        *self.state.worker.borrow_mut() = Some(Worker {
            thread: Some(thread),
            shutdown,
        });
    }

    /// Returns a weak local registration service, not a new admission.
    pub(crate) fn registry(&self) -> Registry {
        self.state.registry.clone()
    }

    /// Polls admitted work in the same turn as consumption, then yields only to its waker.
    pub(crate) async fn answer_next(&mut self) -> bool {
        std::future::poll_fn(|context| self.state.poll_next(context)).await
    }

    /// Ends when all startup, callback and resource admissions close.
    pub(crate) async fn serve(mut self) {
        while self.answer_next().await {}
    }
}

impl State {
    /// Receives one request while servicing every already-ready admitted completion.
    pub(super) fn poll_next(&self, context: &mut Context<'_>) -> Poll<bool> {
        self.poll_next_using(context, &mut Some)
    }
    /// Service scoped operand requests through the original receiver.
    pub(super) fn poll_next_using(
        &self,
        context: &mut Context<'_>,
        service: &mut dyn FnMut(Message) -> Option<Message>,
    ) -> Poll<bool> {
        self.waker.borrow_mut().clone_from(context.waker());
        let mut dispatch = |message| {
            if let Some(message) = service(message) {
                self.dispatch(message);
            }
        };
        loop {
            let message = self.requests.borrow_mut().poll_recv(context);
            match message {
                Poll::Ready(Some(message)) => {
                    dispatch(message);
                    self.poll_pending(context);
                    return Poll::Ready(true);
                }
                Poll::Ready(None) => return Poll::Ready(false),
                Poll::Pending => {}
            }
            if !self.poll_pending(context) {
                return Poll::Pending;
            }
        }
    }

    /// Drains ready completions without rescanning pending futures or inventing wakeups.
    fn poll_pending(&self, context: &mut Context<'_>) -> bool {
        let Ok(mut pending) = self.pending.try_borrow_mut() else {
            return false;
        };
        self.waker.borrow_mut().clone_from(context.waker());
        let mut completed = false;
        loop {
            for work in std::mem::take(&mut *self.staging.borrow_mut()) {
                pending.push(work);
            }
            match pending.poll_next_unpin(context) {
                Poll::Ready(Some(())) => completed = true,
                _ if self.staging.borrow().is_empty() => return completed,
                _ => {}
            }
        }
    }

    /// Answers one request with the native owner it names.
    pub(super) fn dispatch(&self, message: Message) {
        match message {
            Message::ModelStart(request) => request.answer(&self.registry),
            Message::Provider(request) => self.provider_request(request),
            Message::NewStream(request) => request.answer(&self.registry),
            Message::Routed(message, route) => self.routed(*message, route),
            Message::Json(_, request) => request.reject(),
            Message::LockUpdate(_, _, reply) => Self::reject_lock_update(&reply),
            Message::AsyncLockUpdate(id, request) => self.async_update(id, request),
            Message::EndLockUpdate(id) => self.end_lock_update(id),
            Message::SchemaListing(id, reply) => self.reply_schema_listing(id, &reply),
            Message::Completed(id, result) => self.synchronous.complete(id, result),
            Message::Read(id, request) => self.read_context(id, request),
            Message::Api(id, request) => self.api_request(id, request),
            Message::BuiltinOperation(operation) => self.builtin_operation(operation),
            Message::Events(id, request) => self.event(id, request),
            Message::Registration(id, request) => self.api_request(id, request),
            Message::Sleep(request, reply, admission) => self.sleep(request, reply, admission),
            Message::NewSignal(signal, reply) => self.new_signal(signal, &reply),
            Message::OAuthInteraction(request) => self.oauth_interaction(request),
            Message::Effect(id, request) => self.effect(id, request),
            Message::Ui(id, request) => self.ui(id, request),
            Message::Dialog(id, request, admission) => self.dialog(id, request, admission),
            Message::Widgets(message) => self.widgets(message),
            Message::Native(request) => self.native(request),
            Message::Http(request) => self.http(request),
            Message::Watch(id, request) => self.watch(id, request),
            Message::Run(id, call) => self.run(id, call),
            Message::Admit(id, start, admission) => self.start(id, start, admission),
            Message::Release(id) => self.release(id),
        }
    }

    /// Runs a call against its capability, outside the capability-table borrow.
    fn run(&self, id: u64, call: super::RunCall) {
        let capability = self.registry.capability(id);
        call(capability.as_deref(), &self.registry);
    }

    /// Starts the work a call selects against its capability and keeps its admission.
    fn start(&self, id: u64, start: super::AdmitCall, admission: Admission) {
        let capability = self.registry.capability(id);
        self.admit(start(capability.as_deref(), &self.registry), admission);
    }

    /// Rejects a synchronous credential update whose native frame has ended.
    fn reject_lock_update(reply: &std::sync::mpsc::Sender<Result<Option<String>, String>>) {
        let _ = reply.send(Err("extension lock update ended".into()));
    }

    /// Creates the supplied cancellation binding on its original local owner.
    fn new_signal(
        &self,
        signal: Option<Cancellation>,
        reply: &std::sync::mpsc::Sender<Result<super::SignalEndpoint, String>>,
    ) {
        let _ = reply.send(self.registry.signal(signal.unwrap_or_default()));
    }

    /// Releases an ended lock update outside its frame-table borrow.
    fn end_lock_update(&self, id: u64) {
        let frame = self.synchronous.async_frames.borrow_mut().remove(&id);
        drop(frame);
    }

    /// Releases a resource outside its capability-table borrow.
    fn release(&self, id: u64) {
        let removed = self.registry.resources.borrow_mut().entries.remove(&id);
        drop(removed);
    }

    /// Answers a request while its native code owns the route into this instance.
    fn routed(
        &self,
        message: Message,
        route: std::sync::mpsc::Sender<crate::import_route::Nested>,
    ) {
        self.routes.borrow_mut().push(route);
        self.dispatch(message);
        self.routes.borrow_mut().pop();
    }

    /// Sends the original live schema listing to its waiting reader.
    fn reply_schema_listing(
        &self,
        id: u64,
        reply: &std::sync::mpsc::Sender<Result<crate::schema_registration::ToolListing, String>>,
    ) {
        let _ = reply.send(self.schema_listing(id));
    }

    /// Observe live metadata through its original API capability.
    fn schema_listing(&self, id: u64) -> Result<crate::schema_registration::ToolListing, String> {
        let capability = self.registry.capability(id);
        match capability.as_deref() {
            Some(Capability::Api(api)) => self.registry.schema_listing(api),
            _ => Err("extension API resource ended".into()),
        }
    }

    /// Selects the original API once for reads and callback-bearing registrations.
    fn api_request(&self, id: u64, request: impl crate::api_reads::ApiRequest) {
        let capability = self.registry.capability(id);
        if let Some(Capability::Api(api)) = capability.as_deref() {
            request.answer(api, &self.registry);
        } else {
            request.reject("extension API resource ended");
        }
    }

    /// Uses the original bus and admits its work on the existing pending set.
    fn event(&self, id: u64, request: crate::event_reads::Request) {
        let capability = self.registry.capability(id);
        if let Some(Capability::Api(api)) = capability.as_deref() {
            if let Some((work, admission)) = request.answer(api, &self.registry) {
                self.admit(work, admission);
            }
        } else {
            request.reject("extension API resource ended");
        }
    }

    /// Reads the guarded context after releasing the capability table borrow.
    fn read_context(&self, id: u64, request: Request) {
        let capability = self.registry.capability(id);
        if let Some(Capability::Context(owner)) = capability.as_deref() {
            owner.answer(request, &self.registry);
        } else {
            request.reject("extension context resource ended");
        }
    }

    /// Retains the same admission while an already-selected operation settles.
    pub(super) fn admit(&self, work: Work, admission: Admission) {
        self.stage(Box::pin(async move {
            work.await;
            drop(admission);
        }));
    }

    /// Polls work once now and keeps it pending without an admission of its own.
    pub(super) fn stage(&self, mut work: Work) {
        let waker = self.waker.borrow().clone();
        let mut context = Context::from_waker(&waker);
        if work.as_mut().poll(&mut context).is_pending() {
            self.staging.borrow_mut().push(work);
        }
    }

    /// Serves retained UI methods after releasing the capability table borrow.
    fn ui(&self, id: u64, request: crate::ui_reads::Request) {
        let capability = self.registry.capability(id);
        if let Some(Capability::Ui(ui)) = capability.as_deref() {
            request.answer(ui, &self.registry);
        } else {
            request.reject("extension UI resource ended");
        }
    }
}

impl Drop for State {
    fn drop(&mut self) {
        self.requests.get_mut().close();
        let entries = std::mem::take(&mut self.registry.resources.borrow_mut().entries);
        drop(entries);
    }
}

mod retained;
