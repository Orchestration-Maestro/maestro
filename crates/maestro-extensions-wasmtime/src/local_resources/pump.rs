//! Shared local ownership of the sole request receiver and admitted work.
use super::{Admission, Capability, Message, Registry, Work};
use crate::context_reads::{Endpoint, Owner, Request};
use futures_util::{StreamExt, stream::FuturesUnordered};
use maestro_extensions::{Cancellation, Model, ReadonlySessionManager, ResolvedRequestAuth};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::Arc,
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
    /// Set while answering a request its Store thread waits for, when it cannot enter the guest.
    pub(super) store_blocked: std::cell::Cell<bool>,
}

impl Pump {
    /// Separates startup admission from the local owner without spawning work.
    pub(crate) fn new() -> (Self, Admission) {
        let (requests, receiver) = unbounded_channel();
        let state = Rc::new_cyclic(|state| State {
            registry: Registry {
                resources: Rc::default(),
                requests: requests.downgrade(),
                pump: state.clone(),
            },
            requests: RefCell::new(receiver),
            pending: RefCell::new(FuturesUnordered::new()),
            staging: RefCell::default(),
            waker: RefCell::new(std::task::Waker::noop().clone()),
            synchronous: super::synchronous::Returns::default(),
            store_blocked: std::cell::Cell::default(),
        });
        (
            Self { state },
            Admission {
                _requests: requests,
            },
        )
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
        self.waker.borrow_mut().clone_from(context.waker());
        loop {
            let message = self.requests.borrow_mut().poll_recv(context);
            match message {
                Poll::Ready(Some(message)) => {
                    self.answer(message);
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

    /// Dispatches a consumed request outside capability-table borrows.
    fn answer(&self, message: Message) {
        let blocked = self.store_blocked.get();
        self.store_blocked.set(blocked || message.blocks_store());
        self.dispatch(message);
        self.store_blocked.set(blocked);
    }

    /// Answers one request with the native owner it names.
    fn dispatch(&self, message: Message) {
        match message {
            Message::Completed(id, result) => self.synchronous.complete(id, result),
            Message::Read(id, request) => self.read_context(id, request),
            Message::Api(id, request) => self.read_api(id, request),
            Message::Events(id, request) => self.event(id, request),
            Message::Registration(id, request) => self.registration(id, request),
            Message::ApiModel(id, model, reply, admission) => {
                self.start_api_model(id, *model, reply, admission);
            }
            Message::Exec(id, request, reply, admission) => {
                self.start_exec(id, request, reply, admission);
            }
            Message::Command(id, request, admission) => self.start_command(id, request, admission),
            Message::Auth(id, model, reply, admission) => {
                self.start_auth(id, *model, reply, admission);
            }
            Message::NewSignal(reply) => {
                let _ = reply.send(self.registry.signal(Cancellation::new()));
            }
            Message::Signal(id, request) => self.signal(id, request),
            Message::History(id, request) => {
                let capability = self.registry.capability(id);
                if let Some(Capability::History(history)) = capability.as_deref() {
                    request.answer(&*history.borrow(), &self.registry);
                } else {
                    request.reject("extension history resource ended");
                }
            }
            Message::Writer(id, request) => self.write(id, request),
            Message::WriterReader(id, reply) => self.writer_reader(id, &reply),
            Message::Catalog(id, request) => {
                let capability = self.registry.capability(id);
                if let Some(Capability::Catalog(catalog)) = capability.as_deref() {
                    request.answer(catalog);
                } else {
                    request.reject("extension catalog resource ended");
                }
            }
            Message::Node(id, request) => {
                let capability = self.registry.capability(id);
                if let Some(Capability::Node(node)) = capability.as_deref() {
                    node.answer(request, &self.registry);
                } else {
                    request.reject("extension tree node resource ended");
                }
            }
            Message::Replaced(id, request, admission) => {
                self.start_replaced(id, request, admission);
            }
            Message::ReplacedCommand(id, reply) => self.replaced_command(id, &reply),
            Message::Effect(id, request) => self.effect(id, request),
            Message::Ui(id, request) => self.ui(id, request),
            Message::BashOutput(id, bytes, reply) => self.bash_output(id, bytes, &reply),
            Message::Release(id) => {
                let removed = self.registry.resources.borrow_mut().entries.remove(&id);
                drop(removed);
            }
        }
    }

    /// Starts the original creating session's operation outside the registry borrow.
    fn start_replaced(
        &self,
        id: u64,
        request: crate::replaced_reads::Request,
        admission: Admission,
    ) {
        let capability = self.registry.capability(id);
        if let Some(Capability::Replaced(context)) = capability.as_deref() {
            self.admit(request.start(context), admission);
        } else {
            request.reject("extension replaced-session resource ended");
        }
    }

    /// Gives the original guarded command view its own independently retained identity.
    fn replaced_command(&self, id: u64, reply: &std::sync::mpsc::Sender<Result<Endpoint, String>>) {
        let capability = self.registry.capability(id);
        let result = match capability.as_deref() {
            Some(Capability::Replaced(context)) => {
                self.registry.context(Owner::Command((**context).clone()))
            }
            _ => Err("extension replaced-session resource ended".to_owned()),
        };
        let _ = reply.send(result);
    }

    /// Selects a real API outside the capability-table borrow.
    fn read_api(&self, id: u64, request: crate::api_reads::Request) {
        let capability = self.registry.capability(id);
        if let Some(Capability::Api(api)) = capability.as_deref() {
            request.answer(api);
        } else {
            request.reject("extension API resource ended");
        }
    }

    /// Registers native records without holding the capability-table borrow.
    fn registration(&self, id: u64, request: crate::registration_reads::Request) {
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

    /// Selects the original facade and cancellation before starting admitted execution.
    fn start_exec(
        &self,
        id: u64,
        request: crate::exec::Request,
        reply: tokio::sync::oneshot::Sender<Result<String, String>>,
        admission: Admission,
    ) {
        let capability = self.registry.capability(id);
        let Some(Capability::Api(api)) = capability.as_deref() else {
            let _ = reply.send(Err("extension API resource ended".to_owned()));
            return;
        };
        let signal = request
            .signal
            .as_ref()
            .map(|signal| self.registry.cancellation(signal))
            .transpose();
        match signal {
            Ok(signal) => self.admit(
                crate::exec::start(request.data, api.clone(), signal, reply),
                admission,
            ),
            Err(error) => {
                let _ = reply.send(Err(error));
            }
        }
    }

    /// Starts the native model operation at consumption and retains its admission.
    fn start_api_model(
        &self,
        id: u64,
        model: Model,
        reply: tokio::sync::oneshot::Sender<Result<bool, String>>,
        admission: Admission,
    ) {
        let capability = self.registry.capability(id);
        let Some(Capability::Api(api)) = capability.as_deref() else {
            let _ = reply.send(Err("extension API resource ended".to_owned()));
            return;
        };
        match api.set_model(Arc::new(std::sync::RwLock::new(model))) {
            Ok(work) => self.admit(crate::command_reads::settle(work, reply), admission),
            Err(error) => {
                let _ = reply.send(Err(error.to_string()));
            }
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

    /// Selects command work now and keeps its original admission through settlement.
    fn start_command(&self, id: u64, request: crate::command_reads::Request, admission: Admission) {
        let capability = self.registry.capability(id);
        if let Some(Capability::Context(Owner::Command(context))) = capability.as_deref() {
            let work = request.start(context, &self.registry);
            self.admit(work, admission);
        } else {
            request.reject("extension command context resource ended");
        }
    }

    /// Retains the same admission while an already-selected operation settles.
    fn admit(&self, work: Work, admission: Admission) {
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

    /// Retains the original catalog and caller effects across credential resolution.
    fn start_auth(
        &self,
        id: u64,
        model: Model,
        reply: tokio::sync::oneshot::Sender<Result<ResolvedRequestAuth, String>>,
        admission: Admission,
    ) {
        let capability = self.registry.capability(id);
        let Some(Capability::Catalog(catalog)) = capability.as_deref() else {
            let _ = reply.send(Err("extension catalog resource ended".to_owned()));
            return;
        };
        let catalog = catalog.clone();
        let model = Arc::new(std::sync::RwLock::new(model));
        self.admit(
            Box::pin(async move {
                let result = catalog.get_api_key_and_headers(&model).await;
                let _ = reply.send(Ok(result));
            }),
            admission,
        );
    }

    /// Serves retained UI methods after releasing the capability table borrow.
    fn ui(&self, id: u64, request: crate::ui_reads::Request) {
        let capability = self.registry.capability(id);
        if let Some(Capability::Ui(ui)) = capability.as_deref() {
            request.answer(ui);
        } else {
            request.reject("extension UI resource ended");
        }
    }

    /// Applies a mutation to the original setup owner, never to its reader.
    fn write(&self, id: u64, request: crate::writer_reads::Request) {
        let capability = self.registry.capability(id);
        if let Some(Capability::Writer(session)) = capability.as_deref() {
            request.answer(&mut session.borrow_mut());
        } else {
            request.reject("extension writable session resource ended");
        }
    }

    /// Gives the reader its own identity before releasing the writer's capability.
    fn writer_reader(
        &self,
        id: u64,
        reply: &std::sync::mpsc::Sender<Result<crate::history_reads::Endpoint, String>>,
    ) {
        let capability = self.registry.capability(id);
        let result = match capability.as_deref() {
            Some(Capability::Writer(session)) => {
                let reader: Rc<RefCell<dyn ReadonlySessionManager>> = session.clone();
                self.registry.history(reader)
            }
            _ => Err("extension writable session resource ended".to_owned()),
        };
        let _ = reply.send(result);
    }
}

impl Drop for State {
    fn drop(&mut self) {
        self.requests.get_mut().close();
        let entries = std::mem::take(&mut self.registry.resources.borrow_mut().entries);
        drop(entries);
    }
}
