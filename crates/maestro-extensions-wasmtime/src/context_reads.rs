//! Typed primitive context requests between the Store thread and its local owner.
use crate::local_resources::{Lease, Registry, SignalEndpoint, UiEndpoint};
use maestro_extensions::{ContextUsage, ExtensionCommandContext, ExtensionContext};
use std::sync::{Arc, mpsc};

/// A catchable canonical import result after leaving the local owner.
type Reply<T> = mpsc::Sender<Result<T, String>>;

/// Reads or starts an operation on the supplied live context, never on loading defaults.
pub(crate) enum Request {
    /// Reads the context's authored working directory.
    Cwd(Reply<String>),
    /// Retains the selected UI independently of this guarded context.
    Ui(Reply<UiEndpoint>),
    /// Retains the selected cancellation independently of this guarded context.
    Signal(Reply<Option<SignalEndpoint>>),
    /// Retains the original live history independently of this guarded context.
    SessionManager(Reply<crate::history_reads::Endpoint>),
    /// Retains the original live model catalog independently of this context.
    ModelRegistry(Reply<crate::catalog_reads::Endpoint>),
    /// Reads an owned model through the reader captured at context creation.
    Model(Reply<Option<String>>),
    /// Reads the effective prompt at consumption time.
    SystemPrompt(Reply<String>),
    /// Reads whether a UI is installed.
    HasUi(Reply<bool>),
    /// Reads the current idle policy.
    IsIdle(Reply<bool>),
    /// Reads the current pending-message policy.
    HasPendingMessages(Reply<bool>),
    /// Starts the selected abort operation.
    Abort(Reply<()>),
    /// Starts the selected shutdown operation.
    Shutdown(Reply<()>),
    /// Starts native compaction with real completion and error callbacks.
    Compact(crate::compact_calls::Compact, Reply<()>),
    /// Reads the current usage estimate without inventing absent values.
    Usage(Reply<Option<ContextUsage>>),
}

impl Request {
    /// Settles an escaped resource request rather than leaving its Store caller waiting.
    pub(crate) fn reject(self, error: &str) {
        match self {
            Self::Cwd(reply) | Self::SystemPrompt(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::HasUi(reply) | Self::IsIdle(reply) | Self::HasPendingMessages(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::Abort(reply) | Self::Shutdown(reply) | Self::Compact(_, reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::Usage(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::Ui(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::Signal(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::SessionManager(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::ModelRegistry(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::Model(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
        }
    }
}

/// The original local context retained for one canonical resource's lifetime.
pub(crate) enum Owner {
    /// An ordinary event or shortcut context.
    Base(ExtensionContext),
    /// A command context, including its session-control operations.
    Command(ExtensionCommandContext),
}

impl Owner {
    /// Borrows the common view without replacing the original context.
    fn base(&self) -> &ExtensionContext {
        match self {
            Self::Base(context) => context,
            Self::Command(context) => context,
        }
    }

    /// Serializes the model through its native read guard, without copying it.
    fn model(&self) -> Result<Option<String>, String> {
        self.base()
            .model()
            .map_err(|error| error.to_string())?
            .map(|model| {
                let model = model.read().map_err(|error| error.to_string())?;
                serde_json::to_string(&*model).map_err(|error| error.to_string())
            })
            .transpose()
    }

    /// Retains the selected optional signal without extending guarded context validity.
    fn signal(&self, registry: &Registry) -> Result<Option<SignalEndpoint>, String> {
        self.base()
            .signal()
            .map_err(|error| error.to_string())?
            .map(|signal| registry.signal(signal))
            .transpose()
    }

    /// Runs each synchronous domain operation before answering its Store request.
    pub(crate) fn answer(&self, request: Request, registry: &Registry) {
        let context = self.base();
        match request {
            Request::Signal(reply) => {
                let _ = reply.send(self.signal(registry));
            }
            Request::SessionManager(reply) => {
                let result = context
                    .session_manager()
                    .map_err(|error| error.to_string())
                    .and_then(|history| registry.history(history));
                let _ = reply.send(result);
            }
            Request::ModelRegistry(reply) => {
                let result = context
                    .model_registry()
                    .map_err(|error| error.to_string())
                    .and_then(|catalog| registry.catalog(catalog));
                let _ = reply.send(result);
            }
            Request::Model(reply) => {
                let _ = reply.send(self.model());
            }
            Request::Ui(reply) => {
                let result = context
                    .ui()
                    .map_err(|error| error.to_string())
                    .and_then(|ui| registry.ui(ui));
                let _ = reply.send(result);
            }
            Request::Cwd(reply) => {
                let _ = reply.send(context.cwd().map(str::to_owned).map_err(|e| e.to_string()));
            }
            Request::SystemPrompt(reply) => {
                let _ = reply.send(context.get_system_prompt().map_err(|e| e.to_string()));
            }
            Request::HasUi(reply) => {
                let _ = reply.send(context.has_ui().map_err(|e| e.to_string()));
            }
            Request::IsIdle(reply) => {
                let _ = reply.send(context.is_idle().map_err(|e| e.to_string()));
            }
            Request::HasPendingMessages(reply) => {
                let _ = reply.send(context.has_pending_messages().map_err(|e| e.to_string()));
            }
            Request::Abort(reply) => {
                let _ = reply.send(context.abort().map_err(|e| e.to_string()));
            }
            Request::Shutdown(reply) => {
                let _ = reply.send(context.shutdown().map_err(|e| e.to_string()));
            }
            Request::Compact(options, reply) => {
                let result = options.native(registry).and_then(|options| {
                    context.compact(options).map_err(|error| error.to_string())
                });
                let _ = reply.send(result);
            }
            Request::Usage(reply) => {
                let _ = reply.send(context.get_context_usage().map_err(|e| e.to_string()));
            }
        }
    }
}

/// Send-only endpoint retained by the Store rather than an unsendable domain context.
#[derive(Clone)]
pub(crate) struct Endpoint(pub(crate) Arc<Lease>);

impl Endpoint {
    /// Awaits admitted work without blocking the Store's concurrent accessor.
    pub(crate) async fn command<T>(
        &self,
        request: impl FnOnce(
            tokio::sync::oneshot::Sender<Result<T, String>>,
        ) -> crate::command_reads::Request,
    ) -> Result<T, String> {
        self.0
            .request_async(|id, reply, admission| {
                crate::local_resources::Message::Command(id, request(reply), admission)
            })
            .await
    }
}

#[cfg(test)]
mod tests;
