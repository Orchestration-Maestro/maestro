//! A connection's lifetime: binding each session to the protocol, the extension operations
//! behind command contexts, termination signals and shutdown.
use super::connection::{Connection, write_to};
use super::{RpcConnectionOutcome, RpcModeOptions, RpcOutput, RpcRecord};
use crate::agent_session::ExtensionBindings;
use maestro_extensions::EventError;
use maestro_extensions::types::ExtensionUIContext;
use maestro_models::DiagnosticErrorInfo;
use maestro_process::TerminationSignal;
use std::{io, rc::Rc};
use tokio::sync::oneshot;

/// Receives how the connection ended.
pub(super) type Finish = oneshot::Receiver<io::Result<RpcConnectionOutcome>>;

/// Where a connection is in its lifetime.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Phase {
    /// Lines are read; the initial binding may still run.
    #[default]
    Serving,
    /// The input ended while the initial binding runs, so its dialogs cannot be answered.
    InputEnded,
    /// A shutdown owns the teardown but has not yet stopped signal delivery.
    CloseQueued,
    /// Signal delivery and the subscription are stopped and the runtime is disposing.
    Closing,
    /// The connection reported how it ended.
    Closed,
}

/// Shutdown and subscription state of a connection.
#[derive(Default)]
pub(super) struct Lifecycle {
    /// Removes the current session's event subscription.
    unsubscribe: Option<Box<dyn Fn()>>,
    /// Stops signal delivery.
    signal_cleanup: Option<Box<dyn Fn()>>,
    /// Whether an extension requested shutdown.
    shutdown_requested: bool,
    /// Where the connection is in its lifetime.
    phase: Phase,
    /// The first input or output failure, which the connection reports however it ends.
    failure: Option<io::Error>,
    /// Reports how the connection ended; taken by the first report.
    finish: Option<oneshot::Sender<io::Result<RpcConnectionOutcome>>>,
}

impl Connection {
    /// Starts reading immediately; a supplied session binds on its own local task.
    pub(super) fn start(output: RpcOutput, options: RpcModeOptions) -> (Rc<Self>, Finish) {
        let (finish, finished) = oneshot::channel();
        let lifecycle = Lifecycle {
            finish: Some(finish),
            ..Lifecycle::default()
        };
        let connection = Self::new(output, options, lifecycle);
        let Some(session) = &connection.session else {
            connection.early.borrow_mut().take();
            return (connection, finished);
        };
        let weak = Rc::downgrade(&connection);
        session
            .options
            .runtime
            .set_rebind_session(Some(Rc::new(move |_session| {
                let connection = weak.upgrade();
                Box::pin(async move {
                    match connection {
                        Some(connection) => connection.bind_current().await,
                        None => Ok(()),
                    }
                })
            })));
        let initial = Rc::clone(&connection);
        drop(tokio::task::spawn_local(async move {
            initial.finish_initial_binding().await;
        }));
        (connection, finished)
    }

    /// Binds the first session, registers the signals, then handles the records that
    /// arrived meanwhile in their arrival order; a failure ends the connection instead.
    async fn finish_initial_binding(self: &Rc<Self>) {
        let bound = match self.bind_current().await {
            Ok(()) => self.register_signals(),
            Err(error) => Err(io::Error::other(error)),
        };
        let early = self.early.borrow_mut().take();
        match bound {
            Ok(()) => early
                .into_iter()
                .flatten()
                .for_each(|record| self.spawn(record)),
            Err(error) => self.fail(error),
        }
        self.bound.notify_waiters();
    }

    /// Waits until the initial binding has handled the records that arrived meanwhile. The
    /// connection is ending, so the dialogs the binding waits on, now or later, settle as
    /// cancelled: no answer can arrive.
    pub(super) async fn initial_binding(&self) {
        let bound = self.bound.notified();
        if self.early.borrow().is_some() {
            self.lifecycle.borrow_mut().end_input();
            if let Some(session) = &self.session {
                session.ui.cancel_pending();
            }
            bound.await;
        }
    }

    /// Whether a dialog asked now can be answered: not while an ending connection waits for
    /// its initial binding.
    pub(super) fn admits_dialogs(&self) -> bool {
        self.early.borrow().is_none() || self.lifecycle.borrow().phase == Phase::Serving
    }

    /// Whether a shutdown owns the teardown.
    pub(super) fn shutting_down(&self) -> bool {
        matches!(
            self.lifecycle.borrow().phase,
            Phase::CloseQueued | Phase::Closing | Phase::Closed
        )
    }

    /// Binds the current session's extensions to the protocol, then replaces the event
    /// subscription with one on that session.
    async fn bind_current(self: &Rc<Self>) -> Result<(), DiagnosticErrorInfo> {
        let Some(owner) = &self.session else {
            return Ok(());
        };
        let session = owner.session();
        session.bind_extensions(self.bindings(owner)).await?;
        let previous = self.lifecycle.borrow_mut().unsubscribe.take();
        if let Some(unsubscribe) = previous {
            unsubscribe();
        }
        let weak = Rc::downgrade(self);
        let unsubscribe = session.subscribe(Rc::new(move |event| {
            if let Some(connection) = weak.upgrade() {
                drop(connection.write(RpcRecord::Event(event)));
            }
        }));
        self.lifecycle.borrow_mut().unsubscribe = Some(Box::new(unsubscribe));
        Ok(())
    }

    /// The protocol's extension interface, command operations, shutdown request and
    /// failure report.
    fn bindings(
        self: &Rc<Self>,
        session: &super::connection::SessionConnection,
    ) -> ExtensionBindings {
        let (weak, writer) = (Rc::downgrade(self), Rc::downgrade(self));
        ExtensionBindings {
            ui_context: Some(ExtensionUIContext::new(Rc::clone(&session.ui) as Rc<_>).into()),
            command_context_actions: Some(session.options.runtime.command_context_actions()),
            abort_handler: None,
            shutdown_handler: Some(Rc::new(move || {
                if let Some(connection) = weak.upgrade() {
                    connection.request_shutdown();
                }
                Ok(())
            })),
            on_error: Some(Rc::new(move |error| {
                write_to(&writer, RpcRecord::ExtensionError(error)).map_err(EventError::from)
            })),
        }
    }

    /// Delivers termination signals: tracked children are killed, then the connection
    /// shuts down.
    fn register_signals(self: &Rc<Self>) -> io::Result<()> {
        let Some(session) = &self.session else {
            return Ok(());
        };
        let weak = Rc::downgrade(self);
        let cleanup = session.options.process.register_signals(Rc::new(
            move |signal: TerminationSignal| {
                let Some(connection) = weak.upgrade() else {
                    return;
                };
                maestro_tools::utils::shell::kill_tracked_detached_children();
                drop(tokio::task::spawn_local(async move {
                    connection
                        .shutdown(RpcConnectionOutcome::Signal(signal))
                        .await;
                }));
            },
        ))?;
        self.lifecycle.borrow_mut().signal_cleanup = Some(cleanup);
        Ok(())
    }

    /// Records an extension shutdown and checks it again when an active run settles.
    fn request_shutdown(self: &Rc<Self>) {
        self.lifecycle.borrow_mut().shutdown_requested = true;
        let Some(owner) = &self.session else {
            return;
        };
        let session = owner.session();
        if session.is_idle() {
            return;
        }
        let weak = Rc::downgrade(self);
        drop(tokio::task::spawn_local(async move {
            session.wait_for_idle().await;
            if let Some(connection) = weak.upgrade() {
                connection.check_shutdown_requested().await;
            }
        }));
    }

    /// Starts a requested shutdown after a command or run settlement, only while idle.
    pub(super) async fn check_shutdown_requested(self: &Rc<Self>) {
        if self.lifecycle.borrow().shutdown_requested
            && !self.shutting_down()
            && self
                .session
                .as_ref()
                .is_some_and(|owner| owner.session().is_idle())
        {
            self.shutdown(RpcConnectionOutcome::Requested).await;
        }
    }

    /// Keeps `error` as the connection's failure unless one came first, and starts the
    /// shutdown unless one owns the teardown, which then reports the failure.
    pub(super) fn fail(self: &Rc<Self>, error: io::Error) {
        let starting = {
            let mut lifecycle = self.lifecycle.borrow_mut();
            lifecycle.failure.get_or_insert(error);
            lifecycle.claim_close()
        };
        if starting {
            let connection = Rc::clone(self);
            drop(tokio::task::spawn_local(async move {
                connection.close(RpcConnectionOutcome::Eof).await;
            }));
        }
    }

    /// Shuts down with `outcome`. A shutdown repeated while the runtime disposes finishes at
    /// once; one repeated before that leaves the report to the shutdown that owns the
    /// teardown.
    pub(super) async fn shutdown(self: &Rc<Self>, outcome: RpcConnectionOutcome) {
        let (claimed, phase) = {
            let mut lifecycle = self.lifecycle.borrow_mut();
            (lifecycle.claim_close(), lifecycle.phase)
        };
        if claimed {
            self.close(outcome).await;
        } else if phase == Phase::Closing {
            self.finish(Ok(outcome));
        }
    }

    /// Waits for the initial binding, stops signal delivery and the event subscription,
    /// disposes the runtime and finishes with `outcome`.
    async fn close(self: &Rc<Self>, outcome: RpcConnectionOutcome) {
        self.initial_binding().await;
        let (signal_cleanup, unsubscribe) = {
            let mut lifecycle = self.lifecycle.borrow_mut();
            lifecycle.phase = Phase::Closing;
            (
                lifecycle.signal_cleanup.take(),
                lifecycle.unsubscribe.take(),
            )
        };
        if let Some(cleanup) = signal_cleanup {
            cleanup();
        }
        if let Some(unsubscribe) = unsubscribe {
            unsubscribe();
        }
        let disposed = match &self.session {
            Some(session) => session.options.runtime.dispose().await,
            None => Ok(()),
        };
        self.finish(disposed.map(|()| outcome).map_err(io::Error::other));
    }

    /// Reports how the connection ended: the first failure when there was one, otherwise
    /// `ended`; only the first report counts.
    fn finish(&self, ended: io::Result<RpcConnectionOutcome>) {
        let mut lifecycle = self.lifecycle.borrow_mut();
        let Some(finish) = lifecycle.finish.take() else {
            return;
        };
        lifecycle.phase = Phase::Closed;
        let reported = lifecycle.failure.take().map_or(ended, Err);
        drop(lifecycle);
        drop(finish.send(reported));
    }
}

impl Lifecycle {
    /// The failure that arrived after the connection reported how it ended.
    pub(super) fn take_failure(&mut self) -> Option<io::Error> {
        self.failure.take()
    }

    /// Records that the input can no longer answer the initial binding.
    fn end_input(&mut self) {
        if self.phase == Phase::Serving {
            self.phase = Phase::InputEnded;
        }
    }

    /// Gives the teardown to the caller when no shutdown owns it yet.
    fn claim_close(&mut self) -> bool {
        let claimed = matches!(self.phase, Phase::Serving | Phase::InputEnded);
        if claimed {
            self.phase = Phase::CloseQueued;
        }
        claimed
    }
}
