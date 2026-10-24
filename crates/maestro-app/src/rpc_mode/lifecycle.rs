//! A connection's lifetime: binding each session to the protocol, the extension operations
//! behind command contexts, termination signals and shutdown.
use super::connection::Connection;
use super::ui::RpcUi;
use super::{RpcConnectionOutcome, RpcModeOptions, RpcOutput, write_record};
use crate::agent_session::{AgentSession, ExtensionBindings};
use crate::runtime::agent_session_runtime::AgentSessionRuntime;
use maestro_extensions::types::{ExtensionUIContext, SessionChangeResult};
use maestro_extensions::{EventError, ExtensionCommandContextActions};
use maestro_models::DiagnosticErrorInfo;
use maestro_process::TerminationSignal;
use serde::Serialize;
use std::{
    cell::RefCell,
    io,
    rc::{Rc, Weak},
};
use tokio::sync::oneshot;

/// Receives how the connection ended.
pub(super) type Finish = oneshot::Receiver<io::Result<RpcConnectionOutcome>>;

/// Shutdown and subscription state of a connection.
#[derive(Default)]
pub(super) struct Lifecycle {
    /// Removes the current session's event subscription.
    unsubscribe: Option<Box<dyn Fn()>>,
    /// Stops signal delivery.
    signal_cleanup: Option<Box<dyn Fn()>>,
    /// Whether an extension requested shutdown.
    shutdown_requested: bool,
    /// Whether shutdown started.
    pub(super) shutting_down: bool,
    /// The first input or output failure, which the shutdown reports.
    failure: Option<io::Error>,
    /// Reports how the connection ended; taken by the first report.
    finish: Option<oneshot::Sender<io::Result<RpcConnectionOutcome>>>,
}

/// An extension failure as an uncorrelated record.
#[derive(Serialize)]
#[serde(tag = "type", rename = "extension_error", rename_all = "camelCase")]
struct ExtensionErrorRecord<'a> {
    /// The extension's path.
    extension_path: &'a str,
    /// The failed operation or event.
    event: &'a str,
    /// The failure message.
    error: &'a str,
}

impl Connection {
    /// A connection serving `runtime`: the runtime rebinds through it, and the initial
    /// binding runs on its own task while the connection reads.
    pub(super) fn start(
        runtime: Rc<AgentSessionRuntime>,
        output: RpcOutput,
        options: RpcModeOptions,
    ) -> (Rc<Self>, Finish) {
        let (finish, finished) = oneshot::channel();
        let connection = Rc::new(Self {
            ui: Rc::new(RpcUi::new(Rc::clone(&output), options.theme.clone())),
            runtime,
            output,
            options,
            early: RefCell::new(Some(Vec::new())),
            bound: tokio::sync::Notify::new(),
            lifecycle: RefCell::new(Lifecycle {
                finish: Some(finish),
                ..Lifecycle::default()
            }),
        });
        let weak = Rc::downgrade(&connection);
        connection
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

    /// Waits until the initial binding has handled the records that arrived meanwhile.
    pub(super) async fn initial_binding(&self) {
        let bound = self.bound.notified();
        if self.early.borrow().is_some() {
            bound.await;
        }
    }

    /// Binds the current session's extensions to the protocol, then replaces the event
    /// subscription with one on that session.
    async fn bind_current(self: &Rc<Self>) -> Result<(), DiagnosticErrorInfo> {
        let session = self.session();
        session.bind_extensions(self.bindings()).await?;
        let previous = self.lifecycle.borrow_mut().unsubscribe.take();
        if let Some(unsubscribe) = previous {
            unsubscribe();
        }
        let weak = Rc::downgrade(self);
        let unsubscribe = session.subscribe(Rc::new(move |event| {
            if let Some(connection) = weak.upgrade() {
                connection.write(event);
            }
        }));
        self.lifecycle.borrow_mut().unsubscribe = Some(Box::new(unsubscribe));
        Ok(())
    }

    /// The protocol's extension interface, command operations, shutdown request and
    /// failure report.
    fn bindings(self: &Rc<Self>) -> ExtensionBindings {
        let weak = Rc::downgrade(self);
        let output = Rc::clone(&self.output);
        ExtensionBindings {
            ui_context: Some(ExtensionUIContext::new(Rc::clone(&self.ui) as Rc<_>)),
            command_context_actions: Some(command_actions(Rc::downgrade(&self.runtime))),
            shutdown_handler: Some(Rc::new(move || {
                if let Some(connection) = weak.upgrade() {
                    connection.lifecycle.borrow_mut().shutdown_requested = true;
                }
                Ok(())
            })),
            on_error: Some(Rc::new(move |error| {
                let record = ExtensionErrorRecord {
                    extension_path: &error.extension_path,
                    event: &error.event,
                    error: &error.error,
                };
                write_record(&output, &record).map_err(EventError::from)
            })),
        }
    }

    /// Delivers termination signals: tracked children are killed, then the connection
    /// shuts down.
    fn register_signals(self: &Rc<Self>) -> io::Result<()> {
        let weak = Rc::downgrade(self);
        let cleanup =
            self.options
                .process
                .register_signals(Rc::new(move |signal: TerminationSignal| {
                    let Some(connection) = weak.upgrade() else {
                        return;
                    };
                    maestro_tools::utils::shell::kill_tracked_detached_children();
                    drop(tokio::task::spawn_local(async move {
                        connection
                            .shutdown(RpcConnectionOutcome::Signal(signal))
                            .await;
                    }));
                }))?;
        self.lifecycle.borrow_mut().signal_cleanup = Some(cleanup);
        Ok(())
    }

    /// Shuts down after a command when an extension requested it.
    pub(super) async fn check_shutdown_requested(self: &Rc<Self>) {
        if self.lifecycle.borrow().shutdown_requested {
            self.shutdown(RpcConnectionOutcome::Requested).await;
        }
    }

    /// Ends the connection with `error` through the shutdown, once the initial binding is
    /// done; a running shutdown reports the first failure instead of its outcome.
    pub(super) fn fail(self: &Rc<Self>, error: io::Error) {
        let ending = {
            let mut lifecycle = self.lifecycle.borrow_mut();
            let ending = lifecycle.shutting_down || lifecycle.failure.is_some();
            lifecycle.failure.get_or_insert(error);
            ending
        };
        if !ending {
            let connection = Rc::clone(self);
            drop(tokio::task::spawn_local(async move {
                connection.initial_binding().await;
                connection.shutdown(RpcConnectionOutcome::Eof).await;
            }));
        }
    }

    /// Stops signal delivery and the event subscription, disposes the runtime and finishes
    /// with `outcome`, or with the first failure; a repeated shutdown finishes at once.
    pub(super) async fn shutdown(self: &Rc<Self>, outcome: RpcConnectionOutcome) {
        let (signal_cleanup, unsubscribe) = {
            let mut lifecycle = self.lifecycle.borrow_mut();
            if lifecycle.shutting_down {
                drop(lifecycle);
                self.finish(Ok(outcome));
                return;
            }
            lifecycle.shutting_down = true;
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
        let disposed = self.runtime.dispose().await;
        let failure = self.lifecycle.borrow_mut().failure.take();
        self.finish(match failure {
            Some(error) => Err(error),
            None => disposed.map(|()| outcome).map_err(io::Error::other),
        });
    }

    /// Reports how the connection ended; only the first report counts.
    fn finish(&self, outcome: io::Result<RpcConnectionOutcome>) {
        let finish = self.lifecycle.borrow_mut().finish.take();
        if let Some(finish) = finish {
            drop(finish.send(outcome));
        }
    }
}

/// The current runtime, while the connection's caller keeps it.
fn current(runtime: &Weak<AgentSessionRuntime>) -> Result<Rc<AgentSessionRuntime>, EventError> {
    runtime
        .upgrade()
        .ok_or_else(|| "the session runtime has ended".into())
}

/// The command-context operations, forwarded to the runtime's current owners.
fn command_actions(runtime: Weak<AgentSessionRuntime>) -> ExtensionCommandContextActions {
    let (idle, new, fork, navigate, switch, reload) = (
        runtime.clone(),
        runtime.clone(),
        runtime.clone(),
        runtime.clone(),
        runtime.clone(),
        runtime,
    );
    ExtensionCommandContextActions {
        wait_for_idle: Rc::new(move || {
            let runtime = current(&idle);
            Box::pin(async move {
                runtime?.session().agent().wait_for_idle().await;
                Ok(())
            })
        }),
        new_session: Rc::new(move |options| {
            let runtime = current(&new);
            Box::pin(async move { Ok(runtime?.new_session(options.unwrap_or_default()).await?) })
        }),
        fork: Rc::new(move |entry_id, options| {
            let runtime = current(&fork);
            Box::pin(async move {
                let result = runtime?
                    .fork(&entry_id, options.unwrap_or_default())
                    .await?;
                Ok(SessionChangeResult {
                    cancelled: result.cancelled,
                })
            })
        }),
        navigate_tree: Rc::new(move |target_id, options| {
            let runtime = current(&navigate);
            Box::pin(async move {
                let session: Rc<AgentSession> = runtime?.session();
                let result = session
                    .navigate_tree(&target_id, options.unwrap_or_default())
                    .await?;
                Ok(SessionChangeResult {
                    cancelled: result.cancelled,
                })
            })
        }),
        switch_session: Rc::new(move |session_path, options| {
            let runtime = current(&switch);
            Box::pin(async move {
                let switched = runtime?
                    .switch_session(&session_path, None, options.unwrap_or_default())
                    .await?;
                Ok(switched)
            })
        }),
        reload: Rc::new(move || {
            let runtime = current(&reload);
            Box::pin(async move { Ok(runtime?.session().reload().await?) })
        }),
    }
}
