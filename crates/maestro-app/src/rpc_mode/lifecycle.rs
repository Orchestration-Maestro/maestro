//! A connection's lifetime: binding each session to the protocol, the extension operations
//! behind command contexts, termination signals and shutdown.
use super::connection::Connection;
use super::ui::RpcUi;
use super::{RpcConnectionOutcome, RpcModeOptions, RpcOutput, write_record};
use crate::agent_session::ExtensionBindings;
use crate::runtime::agent_session_runtime::AgentSessionRuntime;
use maestro_extensions::EventError;
use maestro_extensions::types::ExtensionUIContext;
use maestro_models::DiagnosticErrorInfo;
use maestro_process::TerminationSignal;
use serde::Serialize;
use std::{cell::RefCell, io, rc::Rc};
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
    /// arrived meanwhile in their arrival order.
    async fn finish_initial_binding(self: &Rc<Self>) {
        if let Err(error) = self.bind_current().await {
            self.finish(Err(io::Error::other(error)));
            return;
        }
        if let Err(error) = self.register_signals() {
            self.finish(Err(error));
            return;
        }
        let early = self.early.borrow_mut().take();
        for record in early.into_iter().flatten() {
            self.spawn(record);
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
            command_context_actions: Some(self.runtime.command_context_actions()),
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

    /// Stops signal delivery and the event subscription, disposes the runtime and finishes
    /// with `outcome`; a repeated shutdown finishes at once.
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
        self.finish(disposed.map(|()| outcome).map_err(io::Error::other));
    }

    /// Reports how the connection ended; only the first report counts.
    pub(super) fn finish(&self, outcome: io::Result<RpcConnectionOutcome>) {
        let finish = self.lifecycle.borrow_mut().finish.take();
        if let Some(finish) = finish {
            drop(finish.send(outcome));
        }
    }
}
